use std::{
    io::{self, BufRead, BufReader, BufWriter, Write},
    net::{AddrParseError, SocketAddr, TcpListener, TcpStream},
    str::FromStr,
    sync::mpsc::{Receiver, SendError, Sender},
    time::Duration,
};

use log::{debug, error, info, trace};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{Direction, config::StreamerConfig};

use super::{
    cpal::CpalStatus,
    udp::{UdpReceiverCommands, UdpSenderCommands, UdpStatus},
};

pub type StartedStream = (Sender<UdpStatus>, Sender<CpalStatus>);

const MAX_TCP_PACKET_SIZE: usize = 65535;

#[derive(Debug)]
#[deprecated]
pub enum TcpErrors {
    ConstructStreamError,
    ConnectError(io::Error),
    SenderLoopError(io::Error),
    ReceiverLoopError(io::Error),
    StopStreamError,
    UdpStatsSendError(SendError<UdpStatus>),
    CpalStatsSendError(SendError<CpalStatus>),
    StreamWriteError(io::Error),
    StreamReadError(io::Error),
    JsonEncodingError(serde_json::Error),
    JsonDecodingError(serde_json::Error),
    SocketAddrParseError(AddrParseError),
}

impl TcpErrors {
    fn io_error_to_string(err: &io::Error) -> String {
        err.to_string()
    }

    fn serde_error_to_string(err: &serde_json::Error) -> String {
        err.to_string()
    }
}

impl ToString for TcpErrors {
    fn to_string(&self) -> String {
        match self {
            TcpErrors::ConstructStreamError => String::from("Error while constructing stream"),
            TcpErrors::ConnectError(error) => Self::io_error_to_string(error),
            TcpErrors::SenderLoopError(error) => Self::io_error_to_string(error),
            TcpErrors::ReceiverLoopError(error) => Self::io_error_to_string(error),
            TcpErrors::StopStreamError => todo!(),
            TcpErrors::UdpStatsSendError(send_error) => send_error.to_string(),
            TcpErrors::CpalStatsSendError(send_error) => send_error.to_string(),
            TcpErrors::StreamWriteError(error) => Self::io_error_to_string(error),
            TcpErrors::StreamReadError(error) => Self::io_error_to_string(error),
            TcpErrors::JsonEncodingError(error) => Self::serde_error_to_string(error),
            TcpErrors::JsonDecodingError(error) => Self::serde_error_to_string(error),
            TcpErrors::SocketAddrParseError(error) => error.to_string(),
        }
    }
}

/// Shortcut for `Result<T, TcpErrors>``
#[deprecated]
pub type TcpResult<T> = Result<T, TcpErrors>;

/// This enum states the type of the tcp control packet.
/// It gets used when the two instances exchange data
#[derive(Serialize, Deserialize, Debug, PartialEq)]

pub enum TcpControlState {
    #[deprecated]
    Connect,

    /// SampleRate, BufferSize, ChannelCount
    ConnectRequest(u32, u32, usize),
    ConnectResponse(String, u16, usize),
    Disconnect,
    Error(String),

    #[deprecated]
    Endpoint(String, EndpointPayload),
    #[deprecated]
    Ping,
    
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct EndpointPayload {
    channels: u16,
    buffer_size: usize,
}

/// This is the data that gets sent between two instances
#[derive(Serialize, Deserialize, Debug)]
pub struct TcpControlPacket {
    pub state: TcpControlState,
}

/// Contains methods that implement the tcp control functionality
///
/// First, the sender sends a [TcpControlState::Connect] packet with its own configuration
/// and waits for the next packet which must be of [TcpControlState::Endpoint] type.
///
/// On the receiver side, it waits for a [TcpControlState::Connect], starts
/// the Stream and sends back an [TcpControlState::Endpoint] Packet containing
/// the Port for the started stream.
#[deprecated]
pub trait TcpControlFlow {
    /// Helper function to create a new TcpListener
    fn create_new_tcp_listener(addr: SocketAddr) -> TcpResult<TcpListener> {
        TcpListener::bind(addr).map_err(|e| TcpErrors::ConnectError(e))
    }

    /// Helper function to create a new TcpStream
    fn create_new_tcp_stream(addr: SocketAddr) -> TcpResult<TcpStream> {
        TcpStream::connect_timeout(&addr, Duration::from_secs(5))
            .map_err(|e| TcpErrors::ConnectError(e))
    }

    fn stop_serve() {}

    /// Serves the TCP Communication Stack
    fn tcp_serve(
        &mut self,
        //tcp_addr: &str,
        streamer_config: StreamerConfig,
        sender_commands: Option<Receiver<UdpSenderCommands>>,
        receiver_commands: Option<Receiver<UdpReceiverCommands>>,
    ) -> TcpResult<()> {
        let addr = streamer_config
            .clone()
            .program_args
            .network_host
            .unwrap_or(String::from("0.0.0.0:42069"));

        let target = SocketAddr::from_str(&addr).unwrap();
        match streamer_config.direction {
            Direction::Sender => {
                let mut stream = Self::create_new_tcp_stream(target)?;

                info!("connecting to {}", target);
                let _sender_loop = self.sender_loop(
                    target,
                    &mut stream,
                    streamer_config,
                    sender_commands.expect("sender loop needs sender commands"),
                )?;

                //Stop stream if the sender loop quits
                self.stop_stream()?;
            }
            Direction::Receiver => {
                let listener = Self::create_new_tcp_listener(target)?;
                info!("listening to {}", target);
                self.receiver_loop(
                    listener,
                    streamer_config,
                    receiver_commands.expect("receiver loop needs receiver commands"),
                )?;
                self.stop_stream()?;
            }
        }
        Ok(())
    }

    /// This method gets called to start the udp stream
    fn start_stream(&mut self, config: StreamerConfig, target: SocketAddr) -> TcpResult<()>;

    /// This method gets called to stop running udp stream
    fn stop_stream(&self) -> TcpResult<()>;

    /// Read from a given TcpStream with a BufReader
    fn read_from_stream(stream: &mut TcpStream) -> TcpResult<TcpControlPacket> {
        let mut reader = BufReader::new(stream);

        let mut buf = String::new();
        reader
            .read_line(&mut buf)
            .map_err(|e| TcpErrors::StreamReadError(e))?;

        let json = serde_json::from_str(&buf).map_err(|e| TcpErrors::JsonDecodingError(e))?;
        debug!("< Read Packet: {:?}", json);

        Ok(json)
    }

    /// Writes to the specified TcpStream using a BufWriter
    fn write_to_stream(stream: &TcpStream, packet: TcpControlPacket) -> TcpResult<()> {
        let mut buf_writer = BufWriter::new(stream);

        debug!("> Sending Packet: {:?}", packet);

        let json = serde_json::to_vec(&packet).map_err(|e| TcpErrors::JsonEncodingError(e))?;

        buf_writer
            .write_all(&json)
            .map_err(|e| TcpErrors::StreamWriteError(e))?;
        buf_writer
            .write(b"\r\n")
            .map_err(|e| TcpErrors::StreamWriteError(e))?;
        buf_writer
            .flush()
            .map_err(|e| TcpErrors::StreamWriteError(e))?;
        Ok(())
    }

    /// This is the loop that gets called when the mode is set to [Direction::Sender]
    fn sender_loop(
        &mut self,
        target_addr: SocketAddr,
        stream: &mut TcpStream,
        streamer_config: StreamerConfig,
        sender_commands: Receiver<UdpSenderCommands>,
    ) -> TcpResult<()> {
        //stream.set_read_timeout(Some(Duration::from_millis(500)))?;
        stream
            .set_nonblocking(true)
            .map_err(|e| TcpErrors::SenderLoopError(e))?;

        // Start by connecting
        let packet = TcpControlPacket {
            state: TcpControlState::Connect,
        };

        // Send Connection Packet
        Self::write_to_stream(&stream, packet)?;

        let mut framesize_buffer = [0u8; MAX_TCP_PACKET_SIZE];
        trace!("Framesize Buffer Size: {}", framesize_buffer.len());

        loop {
            // read command channel first
            if let Ok(msg) = sender_commands.try_recv() {
                debug!("{:?}", msg);
                match msg {
                    UdpSenderCommands::Stop => {
                        let packet = TcpControlPacket {
                            state: TcpControlState::Disconnect,
                        };

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
                        info!("Connecting to port {}, Payload: {:?}", &e, endpoint_payload);
                        let target = SocketAddr::from_str(e.as_str())
                            .map_err(|e| TcpErrors::SocketAddrParseError(e))?;
                        self.start_stream(streamer_config.clone(), target)?;
                    }
                    TcpControlState::Ping => todo!(),
                    TcpControlState::Disconnect => {
                        //self.stop_stream().unwrap();
                        debug!("disconnecting");
                        break;
                    }
                    TcpControlState::Error(e) => {
                        //error!("{}", )
                        todo!()
                    }
                    TcpControlState::Connect => todo!(),
                    TcpControlState::ConnectRequest(_, _, _) => todo!(),
                    TcpControlState::ConnectResponse(_, _, _) => todo!(),
                }
            }
        }

        Ok(())
    }

    fn send_ping(stream: &TcpStream) -> TcpResult<()> {
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
    ) -> TcpResult<()> {
        //for stream in listener.incoming() {
        let (mut stream, _) = listener
            .accept()
            .map_err(|e| TcpErrors::ReceiverLoopError(e))?;
        //let device = device.clone();
        info!("Connected");

        // Assign a random port for the new Stream
        //let mut stream = stream?;
        let mut own_ip = stream.local_addr().unwrap();

        let mut rng = rand::rng();
        let port = rng.random_range(30000..40000);
        own_ip.set_port(port);

        //stream.set_read_timeout(Some(Duration::from_millis(500)))?;
        stream
            .set_nonblocking(true)
            .map_err(|e| TcpErrors::ReceiverLoopError(e))?;

        let mut framesize_buffer = [0u8; 65535];

        loop {
            if let Ok(msg) = receiver_commands.try_recv() {
                match msg {
                    UdpReceiverCommands::Stop => {
                        let packet = TcpControlPacket {
                            state: TcpControlState::Disconnect,
                        };

                        debug!("{:?}", packet);

                        Self::write_to_stream(&stream, packet)?;
                        break;
                    }
                }
            }

            if let Ok(size) = stream.peek(&mut framesize_buffer) {
                let connection_packet = Self::read_from_stream(&mut stream)?;
                match connection_packet.state {
                    TcpControlState::Connect => {
                        let _streamer = self.start_stream(streamer_config.clone(), own_ip).unwrap();

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
                    }
                    TcpControlState::Error(e) => {
                        error!("{}", e);
                        break;
                    }
                    TcpControlState::ConnectRequest(_, _, _) => todo!(),
                    TcpControlState::ConnectResponse(_, _, _) => todo!(),
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::SocketAddr,
        sync::mpsc::channel,
        time::Duration,
    };

    use threadpool::ThreadPool;

    use crate::{
        Direction,
        args::NewCliArgs,
        config::StreamerConfig,
    };

    use super::{TcpControlFlow, TcpErrors, UdpReceiverCommands, UdpSenderCommands};

    struct TcpCommunication {}
    impl TcpControlFlow for TcpCommunication {
        fn start_stream(
            &mut self,
            _config: StreamerConfig,
            _target: SocketAddr,
        ) -> Result<(), TcpErrors> {
            println!("Creating Debug Stream");
            assert!(true);
            Ok(())
        }

        fn stop_stream(&self) -> Result<(), TcpErrors> {
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
