use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    str::FromStr,
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::{Direction, config::StreamerConfig};

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

    /// Serves the TCP Communication Stack
    fn serve(
        &self,
        //tcp_addr: &str,
        streamer_config: StreamerConfig,
    ) -> std::io::Result<()> {
        let addr = streamer_config.clone().program_args.network_host.unwrap();

        let target = SocketAddr::from_str(&addr).unwrap();
        match streamer_config.direction {
            Direction::Sender => {
                let mut stream = Self::create_new_tcp_stream(target)?;
                println!("connecting to {}", target);
                self.sender_loop(target, &mut stream, streamer_config)?;
            }
            Direction::Receiver => {
                let listener = Self::create_new_tcp_listener(target)?;
                println!("listening to {}", target);
                self.receiver_loop(target, listener, streamer_config)?;
            }
        }
        Ok(())
    }

    /// This method gets called to start the udp stream
    fn start_stream(
        &self,
        config: StreamerConfig,
        target: SocketAddr,
    ) -> anyhow::Result<()>;

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
    ) -> std::io::Result<()> {
        // Start by connecting
        let packet = TcpControlPacket {
            state: TcpControlState::Connect,
        };

        // Send Connection Packet
        Self::write_buffer(&stream, packet)?;

        // Read the answer
        let json = Self::read_buffer(stream)?;

        //dbg!(&json);

        //debug_assert_eq!(json.state, TcpControlState::Endpoint(12345));

        match json.state {
            TcpControlState::Endpoint(e) => {
                dbg!("Connecting to port {}", e);

                // TODO implement packet validation

                //#[cfg(not(debug_assertions))]
                let mut target = target_addr.clone();
                target.set_port(e);

                dbg!("Creating streamer for address: {}", target);
                let _streamer = self.start_stream(streamer_config, target);

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
            }
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
        //device: Arc<Mutex<Device>>,
    ) -> std::io::Result<()> {
        for stream in listener.incoming() {
            //let device = device.clone();
            println!("Connected");

            let mut stream = stream?;

            let connection_packet = Self::read_buffer(&mut stream)?;

            debug_assert_eq!(connection_packet.state, TcpControlState::Connect);

            if connection_packet.state == TcpControlState::Connect {
                //if there is no connection already
                // open new stream
                println!("Opening new Stream");

                //open device

                let _streamer = self.start_stream(streamer_config.clone(), target_addr);

                let packet = TcpControlPacket {
                    #[cfg(debug_assertions)]
                    state: TcpControlState::Endpoint(42069),

                    #[cfg(not(debug_assertions))]
                    state: TcpControlState::Endpoint(42069),
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
    use std::{
        net::SocketAddr,
        time::Duration,
    };

    use threadpool::ThreadPool;

    use crate::{args::NewCliArgs, config::StreamerConfig, Direction};

    use super::TcpControlFlow;

    struct TcpCommunication {

    }
    impl TcpControlFlow for TcpCommunication {
        fn start_stream(
            &self,
            _config: StreamerConfig,
            _target: SocketAddr,
        ) -> anyhow::Result<()> {
            assert!(true);
            Ok(())
        }
    }

    #[test]
    fn test_protocol() {
        let pool = ThreadPool::new(2);

        pool.execute(|| {

            let args = NewCliArgs::default();

            let sconfig = StreamerConfig {
                direction: Direction::Receiver,
                buffer_size: 1024,
                send_stats: false,
                selected_channels: vec![0, 1],
                port: 12345,
                program_args: args,
            };

            let server = TcpCommunication {
            };

            server
                .serve(sconfig)
                .unwrap();
        });

        // Wait a second for server to be started
        std::thread::sleep(Duration::from_secs(1));

        pool.execute(|| {
            let args = NewCliArgs::default();

            let sconfig = StreamerConfig {
                direction: Direction::Sender,
                buffer_size: 1024,
                send_stats: false,
                selected_channels: vec![0, 1],
                port: 12345,
                program_args: args,
            };

            let server = TcpCommunication {
            };

            server
                .serve(sconfig)
                .unwrap();
        });

        pool.join();
    }
}
