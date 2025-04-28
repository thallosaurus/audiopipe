use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    str::FromStr,
};

use cpal::Device;
use serde::{Deserialize, Serialize};

use crate::{
    streamer::{Direction, StreamComponent, Streamer},
    streamer_config::StreamerConfig,
};

pub struct TcpCommunication {
    pub direction: Direction,
}

impl TcpCommunication {
    pub fn serve(
        &self,
        tcp_addr: &str,
        streamer_config: StreamerConfig,
        device: Device,
    ) -> std::io::Result<()> {
        match self.direction {
            Direction::Sender => {
                let mut stream = TcpCommunication::create_new_tcp_stream(tcp_addr)?;
                println!("connecting to {}", tcp_addr);
                self.sender_loop(&mut stream, streamer_config, device)?;
            }
            Direction::Receiver => {
                let listener = TcpCommunication::create_new_tcp_listener(tcp_addr)?;
                println!("listening to {}", tcp_addr);
                self.receiver_loop(listener, streamer_config, device)?;
            }
        }
        Ok(())
    }
}

impl TcpControlFlow for TcpCommunication {
    fn start_stream(
        &self,
        streamer_config: StreamerConfig,
        device: Device,
        target: &str,
    ) -> Box<Streamer> {
        Streamer::construct::<f32>(
            Ipv4Addr::from_str(&target).expect("Invalid Host Address"),
            &device,
            streamer_config,
        )
        .unwrap()
    }
}

/// Contains methods that implement the tcp control functionality
///
/// First, the sender sends a [TcpControlState::Connect] packet with its own configuration
/// and waits for the next packet which must be of [TcpControlState::Endpoint] type.
///
/// On the receiver side, it waits for a [TcpControlState::Connect], starts
/// the Stream and sends back an [TcpControlState::Endpoint] Packet containing
/// the Port for the started stream.
pub trait TcpControlFlow {
    fn create_new_tcp_listener(addr: &str) -> std::io::Result<TcpListener> {
        TcpListener::bind(addr)
    }

    fn create_new_tcp_stream(addr: &str) -> std::io::Result<TcpStream> {
        TcpStream::connect(addr)
    }

    /// This method gets called to start the udp stream
    fn start_stream(&self, config: StreamerConfig, device: Device, target: &str) -> Box<Streamer>;

    fn read_buffer(stream: &mut TcpStream) -> std::io::Result<TcpControlPacket> {
        let mut reader = BufReader::new(stream);

        let mut buf = String::new();
        reader.read_line(&mut buf)?;

        let json = serde_json::from_str(&buf)?;

        dbg!(&json);

        Ok(json)
    }

    fn write_buffer(stream: &TcpStream, packet: TcpControlPacket) -> std::io::Result<()> {
        let mut buf_writer = BufWriter::new(stream);

        let json = serde_json::to_vec(&packet)?;

        buf_writer.write_all(&json)?;
        buf_writer.write(b"\r\n")?;
        buf_writer.flush()?;
        Ok(())
    }

    fn sender_loop(
        &self,
        stream: &mut TcpStream,
        streamer_config: StreamerConfig,
        device: Device,
    ) -> std::io::Result<()> {
        // Start by connecting
        let packet = TcpControlPacket {
            state: TcpControlState::Connect,
        };

        // Send Connection Packet
        Self::write_buffer(&stream, packet)?;

        // Read the answer
        let json = Self::read_buffer(stream)?;

        dbg!(&json);

        debug_assert_eq!(json.state, TcpControlState::Endpoint(12345));

        match json.state {
            TcpControlState::Endpoint(e) => {
                println!("Connecting to port {}", e);

                #[cfg(not(debug_assertions))]
                let streamer = self.start_stream(streamer_config, device);

                //wait until the connection is disconnected or dropped
                loop {
                    let packet = Self::read_buffer(stream)?;
                    dbg!(&packet);

                    match packet.state {
                        TcpControlState::Error => {
                            println!("tcp error occurred");
                        }
                        TcpControlState::Disconnect => {
                            println!("Disconnected");
                            break;
                        }
                        _ => todo!(),
                    }
                }
            }
            _ => todo!(),
        }

        Ok(())
    }

    fn receiver_loop(
        &self,
        listener: TcpListener,
        streamer_config: StreamerConfig,
        device: Device,
    ) -> std::io::Result<()> {
        for stream in listener.incoming() {
            println!("Connected");

            let mut stream = stream?;

            let connection_packet = Self::read_buffer(&mut stream)?;

            debug_assert_eq!(connection_packet.state, TcpControlState::Connect);

            if connection_packet.state == TcpControlState::Connect {
                //if there is no connection already
                // open new stream
                println!("Opening new Stream");

                //open device
                #[cfg(not(debug_assertions))]
                let streamer = self.start_stream(streamer_config, device, "0.0.0.0");

                /*#[cfg(debug_assertions)]
                let _udp_stream: Option<Streamer> = None;

                #[cfg(not(debug_assertions))]
                let _udp_stream: Option<Streamer> = None;*/

                let packet = TcpControlPacket {
                    #[cfg(debug_assertions)]
                    state: TcpControlState::Endpoint(12345),

                    #[cfg(not(debug_assertions))]
                    state: TcpControlState::Endpoint(12345),
                };
                dbg!(&packet);

                // send back endpoint
                Self::write_buffer(&stream, packet)?;

                // then we have to wait until the connection is closed or interrupted

                loop {
                    let packet = Self::read_buffer(&mut stream)?;
                    dbg!(&packet);

                    match packet.state {
                        TcpControlState::Error => {
                            println!("tcp error occurred");
                        }
                        TcpControlState::Disconnect => {
                            println!("Disconnected");
                            break;
                        }
                        _ => todo!(),
                    }
                }

                #[cfg(debug_assertions)]
                break;
            } else {
                // refuse
            }
        }
        Ok(())
    }
}

/// This enum states the type of the tcp control packet.
/// It gets used when the two instances exchange data
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum TcpControlState {
    Connect,
    Endpoint(u16),
    Disconnect,
    Error,
}

/// This is the data that gets sent between two instances
#[derive(Serialize, Deserialize, Debug)]
pub struct TcpControlPacket {
    state: TcpControlState,
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;
    #[cfg(test)]
    mod tests {
        use std::time::Duration;

        use cpal::traits::{DeviceTrait, HostTrait};
        use threadpool::ThreadPool;

        use crate::{
            DEFAULT_PORT,
            control::TcpCommunication,
            streamer::{self, Direction},
            streamer_config::StreamerConfig,
        };

        #[test]
        fn test_protocol() {
            let pool = ThreadPool::new(2);

            pool.execute(|| {
                let host = cpal::default_host();
                let device = host.default_input_device().unwrap();
                let config = device.default_input_config().unwrap();

                let streamer_config = StreamerConfig {
                    direction: streamer::Direction::Sender,
                    channel_count: 1,
                    cpal_config: config.into(),
                    buffer_size: 1024,
                    send_network_stats: true,
                    send_cpal_stats: true,
                    selected_channels: vec![0],
                    port: DEFAULT_PORT,
                };

                let server = TcpCommunication {
                    direction: Direction::Receiver,
                };
                server
                    .serve("127.0.0.1:1234", streamer_config, device)
                    .unwrap();
            });

            // Wait a second for server to be started
            std::thread::sleep(Duration::from_secs(1));

            pool.execute(|| {
                let host = cpal::default_host();
                let device = host.default_input_device().unwrap();
                let config = device.default_input_config().unwrap();

                let streamer_config = StreamerConfig {
                    direction: streamer::Direction::Sender,
                    channel_count: 1,
                    cpal_config: config.into(),
                    buffer_size: 1024,
                    send_network_stats: true,
                    send_cpal_stats: true,
                    selected_channels: vec![0],
                    port: DEFAULT_PORT,
                };

                let client = TcpCommunication {
                    direction: Direction::Sender,
                };
                client
                    .serve("127.0.0.1:1234", streamer_config, device)
                    .unwrap();
            });

            pool.join();
        }
    }
}
