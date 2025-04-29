use std::{
    io::{BufRead, BufReader, BufWriter, Write}, net::{SocketAddr, TcpListener, TcpStream}, str::FromStr, sync::{Arc, Mutex}, time::Duration
};

use cpal::Device;
use serde::{Deserialize, Serialize};

use crate::{
    streamer::{Direction, StreamComponent, Streamer},
    streamer_config::StreamerConfig,
};

/// Provisorial Struct to initialize the TCP Control Flow
#[deprecated]
pub struct TcpCommunication {
    pub direction: Direction,
}

impl TcpCommunication {
    pub fn serve(
        &self,
        tcp_addr: SocketAddr,
        streamer_config: StreamerConfig,
        device: Arc<Mutex<Device>>,
    ) -> std::io::Result<()> {
        match self.direction {
            Direction::Sender => {
                let mut stream = TcpCommunication::create_new_tcp_stream(tcp_addr)?;
                println!("connecting to {}", tcp_addr);
                self.sender_loop(tcp_addr, &mut stream, streamer_config, device)?;
            }
            Direction::Receiver => {
                let listener = TcpCommunication::create_new_tcp_listener(tcp_addr)?;
                println!("listening to {}", tcp_addr);
                self.receiver_loop(tcp_addr, listener, streamer_config, device)?;
            }
        }
        Ok(())
    }
}

impl TcpControlFlow for TcpCommunication {
    fn start_stream(
        &self,
        streamer_config: StreamerConfig,
        device: Arc<Mutex<cpal::Device>>,
        target: SocketAddr,
    ) -> Result<(), anyhow::Error> {
        // TODO Implement more data types
        // locked to f32 for now
        Streamer::construct::<f32>(
            target,
            device,
            streamer_config,
        )
        .unwrap();

    Ok(())
    }
    
    fn get_tcp_direction(&self) -> Direction {
        self.direction
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
    /// Helper function to create a new TcpListener
    fn create_new_tcp_listener(addr: SocketAddr) -> std::io::Result<TcpListener> {
        TcpListener::bind(addr)
    }

    /// Helper function to create a new TcpStream
    fn create_new_tcp_stream(addr: SocketAddr) -> std::io::Result<TcpStream> {
        TcpStream::connect_timeout(&addr, Duration::from_secs(5))
    }

    fn get_tcp_direction(&self) -> Direction;

    fn serve(
        &self,
        tcp_addr: &str,
        streamer_config: StreamerConfig,
        device: Arc<Mutex<Device>>,
    ) -> std::io::Result<()> {
        let target = SocketAddr::from_str(tcp_addr).unwrap();
        match self.get_tcp_direction() {
            Direction::Sender => {
                let mut stream = TcpCommunication::create_new_tcp_stream(target)?;
                println!("connecting to {}", tcp_addr);
                self.sender_loop(target, &mut stream, streamer_config, device)?;
            }
            Direction::Receiver => {
                let listener = TcpCommunication::create_new_tcp_listener(target)?;
                println!("listening to {}", tcp_addr);
                self.receiver_loop(target, listener, streamer_config, device)?;
            }
        }
        Ok(())
    }

    /// This method gets called to start the udp stream
    fn start_stream(&self, config: StreamerConfig, device: Arc<Mutex<Device>>, target: SocketAddr) -> anyhow::Result<()>;

    /// Read from a given TcpStream with a BufReader
    fn read_buffer(stream: &mut TcpStream) -> std::io::Result<TcpControlPacket> {
        let mut reader = BufReader::new(stream);

        let mut buf = String::new();
        reader.read_line(&mut buf)?;

        let json = serde_json::from_str(&buf)?;

        dbg!(&json);

        Ok(json)
    }

    /// Writes to the specified TcpStream using a BufWriter
    fn write_buffer(stream: &TcpStream, packet: TcpControlPacket) -> std::io::Result<()> {
        let mut buf_writer = BufWriter::new(stream);

        let json = serde_json::to_vec(&packet)?;

        buf_writer.write_all(&json)?;
        buf_writer.write(b"\r\n")?;
        buf_writer.flush()?;
        Ok(())
    }

    /// This is the loop that gets called when the mode is set to [Direction::Sender]
    fn sender_loop(
        &self,
        target_addr: SocketAddr,
        stream: &mut TcpStream,
        streamer_config: StreamerConfig,
        device: Arc<Mutex<Device>>,
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

                // TODO implement packet validation

                //#[cfg(not(debug_assertions))]
                let streamer = self.start_stream(streamer_config, device, target_addr);

                //wait until the connection is disconnected or dropped
                loop {
                    let packet = Self::read_buffer(stream)?;
                    dbg!(&packet);

                    match packet.state {
                        TcpControlState::Error => {
                            println!("tcp error occurred");
                            break;
                        }
                        TcpControlState::Disconnect => {
                            println!("Disconnected");
                            break;
                        }
                        _ => todo!(),
                    }
                }
                let packet = TcpControlPacket {
                    state: TcpControlState::Disconnect,
                };
                Self::write_buffer(stream, packet)?;
            }
            _ => {
                // TODO implement error handling here
                todo!()
            },
        }

        Ok(())
    }

    fn send_ping(stream: &TcpStream) -> std::io::Result<()> {
        let packet = TcpControlPacket {
            state: TcpControlState::Ping,
        };

        Self::write_buffer(stream, packet)?;
        Ok(())
    }

    /// This is the loop that gets called when the mode is set to [Direction::Receiver]
    fn receiver_loop(
        &self,
        target_addr: SocketAddr,
        listener: TcpListener,
        streamer_config: StreamerConfig,
        device: Arc<Mutex<Device>>,
    ) -> std::io::Result<()> {
        for stream in listener.incoming() {
            let device = device.clone();
            println!("Connected");

            let mut stream = stream?;

            let connection_packet = Self::read_buffer(&mut stream)?;

            debug_assert_eq!(connection_packet.state, TcpControlState::Connect);

            if connection_packet.state == TcpControlState::Connect {
                //if there is no connection already
                // open new stream
                println!("Opening new Stream");

                //open device

                let streamer = self.start_stream(streamer_config.clone(), device, target_addr);

                let packet = TcpControlPacket {
                    #[cfg(debug_assertions)]
                    state: TcpControlState::Endpoint(12345),

                    #[cfg(not(debug_assertions))]
                    state: TcpControlState::Endpoint(12345),
                };
                dbg!(&packet);

                // send back endpoint
                Self::write_buffer(&stream, packet)?;

                // then we have to wait until the connection is closed or dropped
                loop {
                    let packet = Self::read_buffer(&mut stream)?;
                    dbg!(&packet);

                    match packet.state {
                        TcpControlState::Ping => {
                            //send back pong
                            Self::send_ping(&stream)?;
                        }
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
    Ping,
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
        use std::{net::SocketAddr, str::FromStr, sync::{Arc, Mutex}, time::Duration};

        use cpal::traits::{DeviceTrait, HostTrait};
        use threadpool::ThreadPool;

        use crate::{
            DEFAULT_PORT,
            components::control::TcpCommunication,
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

                let device = Arc::new(Mutex::new(device));
                server
                    .serve(SocketAddr::from_str("127.0.0.1:1234").unwrap(), streamer_config, device)
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

                let device = Arc::new(Mutex::new(device));

                client
                    .serve(SocketAddr::from_str("127.0.0.1:1234").unwrap(), streamer_config, device)
                    .unwrap();
            });

            pool.join();
        }
    }
}
