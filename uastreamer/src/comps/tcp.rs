use std::{
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
    sync::Mutex,
};

use crate::comps::udp::{start_audio_stream_client, start_audio_stream_server, UdpClientHandle, UdpServerHandle};

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

pub async fn tcp_server(
    sock_addr: &str,
    channel_count: usize,
    max_buffer_size: u32,
) -> io::Result<()> {
    let ip: Ipv4Addr = sock_addr.parse().expect("parse failed");
    let target = SocketAddr::new(std::net::IpAddr::V4(ip), 6789);
    let listen = TcpListener::bind(target).await?;

    let handles: Arc<Mutex<HashMap<uuid::Uuid, UdpServerHandle>>> =
        Arc::new(Mutex::new(HashMap::new()));
    info!("Server Listening");

    loop {
        let (mut socket, client_addr) = listen.accept().await?;

        let handles = handles.clone();
        tokio::spawn(async move {
            info!("New connection from {}", client_addr);
            let mut buf = vec![0; 8196];

            loop {
                let n = socket
                    .read(&mut buf)
                    .await
                    .expect("failed to read data from socket");

                let data = &buf[..n];
                trace!("{:?}", data);
                let json: Result<TokioTcpControlPacket, serde_json::Error> =
                    serde_json::from_slice(data);
                debug!("{:?}", json);

                if let Ok(json) = json {
                    match json.state {
                        TokioTcpControlState::ConnectRequest(smprt, bufsize, chcount) => {
                            // open udp socket here
                            let connection_id = uuid::Uuid::new_v4();
                            let mut h = handles.lock().await;
                            let handle = start_audio_stream_server(smprt, bufsize, chcount).await;
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
                            let json = serde_json::to_vec(&connection_response).unwrap();

                            if let Err(e) = socket.write_all(json.as_slice()).await {
                                //remove connection from map
                                let _ = h.remove(&connection_id);
                                error!("r {}", e);
                                break;
                            }
                        }
                        _ => {
                            todo!("Unsupported TCP State")
                        }
                    }
                } else {
                    // eof probably
                    let err = json.err().unwrap();
                    error!("{}", err);
                    break;
                }

                #[cfg(test)]
                break;
            }
        });
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
            TokioTcpControlState::ConnectRequest(samplerate, buffersize, chcount) => {}
            TokioTcpControlState::Disconnect => todo!(),
            TokioTcpControlState::Error(_) => todo!(),
            TokioTcpControlState::ConnectResponse(conn_id, port, chcount) => {
                // TODO
                let mut _h = handle.lock().await;
                if _h.is_none() {
                    let ip: Ipv4Addr = target_node_addr.parse().expect("parse failed");
                    let target = SocketAddr::new(std::net::IpAddr::V4(ip), port);
                    info!("{}", target);
                    *_h = Some(start_audio_stream_client(target, max_buffer_size, chcount).await);
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
    async fn test_client_server_handshake() {
        env_logger::init();
        tokio::spawn(async move {
            // start server here
            //tcp_server("0.0.0.0:6789").await.unwrap();
        });
    }
}
