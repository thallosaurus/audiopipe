use std::{
    cell::RefCell,
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use cpal::StreamConfig;
use log::{debug, error, info, trace};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{
        Mutex, RwLock,
        mpsc::{self, Receiver},
    },
    task::JoinHandle,
};

use crate::async_comp::{
    audio::{GLOBAL_MASTER_OUTPUT_MIXER, RawInputChannel},
    udp::{UdpClientHandle, UdpServerHandle},
};

/// This enum states the type of the tcp control packet.
/// It gets used when the two instances exchange data
#[derive(Serialize, Deserialize, Debug, PartialEq)]

pub enum TokioTcpControlState {
    /// SampleRate, BufferSize, ChannelCount
    ConnectRequest(u32, u32, usize),
    ConnectResponse(String, u16, usize),
    Disconnect,
    Error(String),
}

/// This is the data that gets sent between two instances
#[derive(Serialize, Deserialize, Debug)]
pub struct TokioTcpControlPacket {
    pub state: TokioTcpControlState,
}

type SharedTcpServerHandles = Arc<Mutex<HashMap<uuid::Uuid, UdpServerHandle>>>;

async fn send_packet(stream: &mut TcpStream, packet: TokioTcpControlPacket) -> io::Result<()> {
    let json = serde_json::to_vec(&packet).unwrap();

    stream.write_all(json.as_slice()).await
}

async fn read_packet(
    stream: &mut TcpStream,
    mut buf: &mut [u8],
) -> io::Result<TokioTcpControlPacket> {
    let n = stream.read(&mut buf).await?;

    let data = &buf[..n];
    trace!("{:?}", data);
    let json = serde_json::from_slice(data)?;
    debug!("{:?}", json);
    Ok(json)
}

enum TcpServerCommands {
    Stop,
}

pub struct TcpServer {
    pub _task: JoinHandle<io::Result<()>>,
    channel: mpsc::Sender<TcpServerCommands>,
}

//impl TcpServer {
/// The entry point to the tcp communication server
pub async fn new_control_server(sock_addr: String, channel_count: usize) -> io::Result<()> {
    let ip: Ipv4Addr = sock_addr.parse().expect("parse failed");
    let target = SocketAddr::new(std::net::IpAddr::V4(ip), 6789);

    //let (s, r) = mpsc::channel(1);

    //TcpServer {

    let listen = TcpListener::bind(target).await?;
    let handles: SharedTcpServerHandles = Arc::new(Mutex::new(HashMap::new()));

    //let rc = Arc::new(Mutex::new(r));

    info!("Server Listening");
    loop {
        if let Ok((socket, _client_addr)) = listen.accept().await {
            tokio::spawn(handle_tcp_server_connection(
                socket,
                handles.clone(),
                channel_count,
            ));
        }
    }
}

async fn handle_tcp_server_connection(
    mut socket: TcpStream,
    handles: SharedTcpServerHandles,
    channel_count: usize,
    //cmd: mpsc::Receiver<TcpServerCommands>
    //handle: Fn(u32, u32, usize, (RawInputChannel, RawInputChannel))
) -> io::Result<()> {
    let mut buf = vec![0; 8196];
    //let mixer = GLOBAL_MASTER_OUTPUT_MIXER.clone().lock().await.expect("failed to open master output mixer");
    let mixer = GLOBAL_MASTER_OUTPUT_MIXER.lock().await;

    loop {
        let json = read_packet(&mut socket, &mut buf).await?;

        match json.state {
            TokioTcpControlState::ConnectRequest(smprt, bufsize, chcount) => {
                // open udp socket here
                let connection_id = uuid::Uuid::new_v4();
                let mut h = handles.lock().await;

                let mut mixer = mixer.clone().expect("failed to open master output mixer");

                // TODO choose selected channels here
                let l_ch = mixer.get_channel(0);
                let r_ch = mixer.get_channel(1);

                let handle = UdpServerHandle::start_audio_stream_server(
                    smprt,
                    bufsize,
                    chcount,
                    (l_ch.clone(), r_ch.clone()),
                )
                .await;

                let local_addr = handle.local_addr.clone();
                h.insert(connection_id, handle);

                info!("new udp connection id {}", connection_id);

                let connection_response = TokioTcpControlPacket {
                    state: TokioTcpControlState::ConnectResponse(
                        connection_id.to_string(),
                        local_addr.port(),
                        channel_count,
                    ),
                };
                //let json = serde_json::to_vec(&connection_response).unwrap();
                send_packet(&mut socket, connection_response).await?;
            }
            _ => {
                todo!("Unsupported TCP State")
            }
        }

        #[cfg(test)]
        break;
    }
    Ok(())
}

/// channel count tells us, how many of our available channels we want to send
/// dont get tempted to just pipe in the stream config variable
pub async fn tcp_client(
    target_node_addr: &str,
    config: &StreamConfig,
    channel_count: usize,
    max_buffer_size: u32,
) -> io::Result<()> {
    let ip: Ipv4Addr = target_node_addr.parse().expect("parse failed");
    let target = SocketAddr::new(std::net::IpAddr::V4(ip), 6789);
    let mut stream = TcpStream::connect(target).await?;
    info!("Connected to server {}", target);

    let packet = TokioTcpControlPacket {
        state: TokioTcpControlState::ConnectRequest(
            config.sample_rate.0,
            max_buffer_size,
            channel_count,
        ),
    };

    let mut json = serde_json::to_vec(&packet)?;
    stream.write_all(json.as_mut_slice()).await.unwrap();

    let handle: Arc<Mutex<Option<UdpClientHandle>>> = Arc::new(Mutex::new(None));
    let mut buf = vec![0; 1024];

    loop {
        let n = stream
            .read(&mut buf)
            .await
            .expect("failed to read data from socket");

        let data = &buf[..n];
        let json: TokioTcpControlPacket = serde_json::from_slice(data).unwrap();
        debug!("< Read Packet: {:?}", json);
        match json.state {
            TokioTcpControlState::ConnectRequest(samplerate, buffersize, chcount) => todo!(),
            TokioTcpControlState::Disconnect => {}
            TokioTcpControlState::Error(_) => todo!(),
            TokioTcpControlState::ConnectResponse(conn_id, port, chcount) => {
                let mut _h = handle.lock().await;
                if _h.is_none() {
                    let ip: Ipv4Addr = target_node_addr.parse().expect("parse failed");
                    let target = SocketAddr::new(std::net::IpAddr::V4(ip), port);
                    info!("{}", target);
                    *_h = Some(
                        UdpClientHandle::start_audio_stream_client(
                            target,
                            max_buffer_size,
                            chcount,
                        )
                        .await,
                    );
                } else {
                    // there is already a stream
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_handle_tcp_server_connection() {
        env_logger::init();
    }
}
