use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    str::FromStr,
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{Direction, config::StreamerConfig};

use super::{cpal::CpalStatus, udp::{UdpReceiverCommands, UdpSenderCommands, UdpStatus}};

pub type StartedStream = (Sender<UdpStatus>,Sender<CpalStatus>);

const MAX_TCP_PACKET_SIZE: usize = 65535;

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
    fn tcp_serve(
        &mut self,
        //tcp_addr: &str,
        streamer_config: StreamerConfig,
        sender_commands: Option<Receiver<UdpSenderCommands>>,
        receiver_commands: Option<Receiver<UdpReceiverCommands>>,
    ) -> std::io::Result<()> {
        let addr = streamer_config
            .clone()
            .program_args
            .network_host
            .unwrap_or(String::from("0.0.0.0:42069"));

        let target = SocketAddr::from_str(&addr).unwrap();
        match streamer_config.direction {
            Direction::Sender => {
                let mut stream = Self::create_new_tcp_stream(target)?;
                println!("connecting to {}", target);
                self.sender_loop(
                    target,
                    &mut stream,
                    streamer_config,
                    sender_commands.expect("sender loop needs sender commands"),
                )?;
            }
            Direction::Receiver => {
                let listener = Self::create_new_tcp_listener(target)?;
                println!("listening to {}", target);
                self.receiver_loop(
                    listener,
                    streamer_config,
                    receiver_commands.expect("receiver loop needs receiver commands"),
                )?;
            }
        }
        Ok(())
    }

    /// This method gets called to start the udp stream
    fn start_stream(&mut self, config: StreamerConfig, target: SocketAddr) -> anyhow::Result<StartedStream>;

    /// This method gets called to stop running udp stream
    fn stop_stream(&self) -> anyhow::Result<()>;

    /// Read from a given TcpStream with a BufReader
    fn read_from_stream(stream: &mut TcpStream) -> std::io::Result<TcpControlPacket> {
        let mut reader = BufReader::new(stream);

        let mut buf = String::new();
        reader.read_line(&mut buf)?;

        let json = serde_json::from_str(&buf)?;

        dbg!(&json);

        Ok(json)
    }

    /// Writes to the specified TcpStream using a BufWriter
    fn write_to_stream(stream: &TcpStream, packet: TcpControlPacket) -> std::io::Result<()> {
        let mut buf_writer = BufWriter::new(stream);

        let json = serde_json::to_vec(&packet)?;

        buf_writer.write_all(&json)?;
        buf_writer.write(b"\r\n")?;
        buf_writer.flush()?;
        Ok(())
    }

    /// This is the loop that gets called when the mode is set to [Direction::Sender]
    fn sender_loop(
        &mut self,
        target_addr: SocketAddr,
        stream: &mut TcpStream,
        streamer_config: StreamerConfig,
        sender_commands: Receiver<UdpSenderCommands>,
    ) -> std::io::Result<()> {
        //stream.set_read_timeout(Some(Duration::from_millis(500)))?;
        stream.set_nonblocking(true)?;

        // Start by connecting
        let packet = TcpControlPacket {
            state: TcpControlState::Connect,
        };

        // Send Connection Packet
        Self::write_to_stream(&stream, packet)?;

        let mut framesize_buffer = [0u8; MAX_TCP_PACKET_SIZE];

        loop {
            // read command channel first
            if let Ok(msg) = sender_commands.try_recv() {
                match msg {
                    UdpSenderCommands::Stop => {
                        let packet = TcpControlPacket {
                            state: TcpControlState::Disconnect,
                        };

                        dbg!(&packet);

                        Self::write_to_stream(stream, packet)?;
                        break;
                    }
                }
            }

            // then gather the buffer
            if let Ok(size) = stream.peek(&mut framesize_buffer) {
                let json = Self::read_from_stream(stream)?;

                match json.state {
                    TcpControlState::Endpoint(e, endpoint_payload) => {
                        // connect to receiver
                        println!("Connecting to port {}, Payload: {:?}", &e, endpoint_payload);
                        let target = SocketAddr::from_str(e.as_str()).unwrap();
                        let _streamer = self.start_stream(streamer_config.clone(), target);
                    }
                    TcpControlState::Ping => todo!(),
                    TcpControlState::Disconnect => {
                        break;
                    },
                    TcpControlState::Error => todo!(),
                    TcpControlState::Connect => todo!(),
                }
            }
        }

        Ok(())
    }

    fn send_ping(stream: &TcpStream) -> std::io::Result<()> {
        let packet = TcpControlPacket {
            state: TcpControlState::Ping,
        };

        Self::write_to_stream(stream, packet)?;
        Ok(())
    }

    /// This is the loop that gets called when the mode is set to [Direction::Receiver]
    fn receiver_loop(
        &mut self,
        //target_addr: SocketAddr,
        listener: TcpListener,
        streamer_config: StreamerConfig,
        receiver_commands: Receiver<UdpReceiverCommands>,
        //device: Arc<Mutex<Device>>,
    ) -> std::io::Result<()> {
        //for stream in listener.incoming() {
        let (mut stream, _) = listener.accept()?;
            //let device = device.clone();
            println!("Connected");

            // Assign a random port for the new Stream
            //let mut stream = stream?;
            let mut own_ip = stream.local_addr()?;
            let mut rng = rand::rng();
            let port = rng.random_range(30000..40000);
            own_ip.set_port(port);

            //stream.set_read_timeout(Some(Duration::from_millis(500)))?;
            stream.set_nonblocking(true)?;

            let mut framesize_buffer = [0u8; 65535];

            loop {
                if let Ok(msg) = receiver_commands.try_recv() {
                    match msg {
                        UdpReceiverCommands::Stop => {
                            let packet = TcpControlPacket {
                                state: TcpControlState::Disconnect,
                            };
    
                            dbg!(&packet);
    
                            Self::write_to_stream(&stream, packet)?;
                            break;
                        }
                    }
                }

                if let Ok(size) = stream.peek(&mut framesize_buffer) {
                    let connection_packet = Self::read_from_stream(&mut stream)?;
                    match connection_packet.state {
                        TcpControlState::Connect => {
                            let _streamer =
                                self.start_stream(streamer_config.clone(), own_ip).unwrap();

                            let packet = TcpControlPacket {
                                //#[cfg(debug_assertions)]
                                state: TcpControlState::Endpoint(
                                    own_ip.to_string(),
                                    EndpointPayload {
                                        channels: streamer_config.selected_channels.len() as u16,
                                        buffer_size: streamer_config.buffer_size,
                                    },
                                ),
                            };

                            // send back endpoint
                            Self::write_to_stream(&stream, packet)?;
                        }
                        TcpControlState::Endpoint(_, endpoint_payload) => todo!(),
                        TcpControlState::Ping => todo!(),
                        TcpControlState::Disconnect => {
                            break;
                        },
                        TcpControlState::Error => todo!(),
                    }
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
    Endpoint(String, EndpointPayload),
    Ping,
    Disconnect,
    Error,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct EndpointPayload {
    channels: u16,
    buffer_size: usize,
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
        sync::mpsc::{Sender, channel},
        time::Duration,
    };

    use threadpool::ThreadPool;

    use crate::{args::NewCliArgs, components::{cpal::CpalStatus, udp::UdpStatus}, config::StreamerConfig, Direction};

    use super::{StartedStream, TcpControlFlow, UdpReceiverCommands, UdpSenderCommands};

    struct TcpCommunication {}
    impl TcpControlFlow for TcpCommunication {
        fn start_stream(
            &mut self,
            _config: StreamerConfig,
            _target: SocketAddr,
        ) -> anyhow::Result<StartedStream> {
            println!("Creating Debug Stream");
            assert!(true);

            let (cpal_msg_tx, cpal_msg_rx) = channel::<CpalStatus>();
            let (udp_msg_tx, udp_msg_rx) = channel::<UdpStatus>();
            Ok((udp_msg_tx,cpal_msg_tx))
        }

        fn stop_stream(&self) -> anyhow::Result<()> {
            assert!(true);
            Ok(())
        }
    }

    #[test]
    fn test_protocol() {
        let pool = ThreadPool::new(3);

        let (cmd_tx_recv, cmd_rx) = channel::<UdpReceiverCommands>();
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

            let mut server = TcpCommunication {};

            server.tcp_serve(sconfig, None, Some(cmd_rx)).unwrap();
        });

        // Wait a second for server to be started
        std::thread::sleep(Duration::from_secs(1));

        let (cmd_tx_send, cmd_rx) = channel::<UdpSenderCommands>();

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

            let mut server = TcpCommunication {};

            server.tcp_serve(sconfig, Some(cmd_rx), None).unwrap();
        });

        // Now the thread for control
        pool.execute(move || {
            std::thread::sleep(Duration::from_secs(10));
            println!("Sending stop signal");
            cmd_tx_send.send(UdpSenderCommands::Stop).unwrap();
            cmd_tx_recv.send(UdpReceiverCommands::Stop).unwrap();
        });
        pool.join();
    }
}
