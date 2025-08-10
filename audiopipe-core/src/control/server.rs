use std::{collections::HashMap, net::{Ipv4Addr, SocketAddr}, sync::Arc};

use log::{debug, error, info, trace};
use tokio::{io::{self, AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, sync::Mutex};

use crate::{
    audio::GLOBAL_MASTER_OUTPUT_MIXER,
    control::{
        packet::{ControlError, ControlRequest, ControlResponse}, BufferSize, ChannelCount, ConnectionControl, ConnectionControlResult, ConnectionControlState, Port, SharedUdpServerHandles
    },
    mixer::MixerTrait, streamer::receiver::UdpServerHandle,
};

async fn send_packet(stream: &mut TcpStream, packet: ControlResponse) -> io::Result<()> {
    let json = serde_json::to_vec(&packet).unwrap();

    stream.write_all(json.as_slice()).await
}

async fn read_packet(stream: &mut TcpStream, mut buf: &mut [u8]) -> io::Result<ControlRequest> {
    let n = stream.read(&mut buf).await?;

    let data = &buf[..n];
    trace!("{:?}", data);
    let json = serde_json::from_slice(data)?;
    debug!("{:?}", json);
    Ok(json)
}

/// The entry point to the tcp communication server
pub async fn new_control_server(sock_addr: String) -> io::Result<()> {
    let ip: Ipv4Addr = sock_addr.parse().expect("parse failed");
    let target = SocketAddr::new(std::net::IpAddr::V4(ip), 6789);

    let listen = TcpListener::bind(target).await?;

    // holds all open udp audio streams
    let handles = Arc::new(Mutex::new(HashMap::new()));

    //let rc = Arc::new(Mutex::new(r));

    info!("Server Listening");
    loop {
        if let Ok((mut socket, _client_addr)) = listen.accept().await {

            // copy handle for udp streams
            let handles = Arc::clone(&handles);

            // spawn a new task for the connection
            tokio::spawn(async move {
                let mut buf = vec![0; 8196];
                //let handles = child_handles.lock().await;
                loop {
                    let handles = Arc::clone(&handles);
                    
                    // TODO tcp error handling
                    match read_packet(&mut socket, &mut buf).await.unwrap() {
                        ControlRequest::OpenStream(mixer_track_selector) => {
                            let connection_id = uuid::Uuid::new_v4();
                            
                            let mixer = GLOBAL_MASTER_OUTPUT_MIXER.lock().await;
                            let mixer = mixer.as_ref().expect("failed to open mixer");

                            if let Ok(channel) = mixer.get_channel(mixer_track_selector) {
                                let handle =
                                    UdpServerHandle::start_audio_stream_server(channel).await;

                                let local_addr = handle.local_addr.clone();
                                handles.lock().await.insert(connection_id, handle);

                                info!("new udp connection id {}", connection_id);

                                /*let connection_response = ConnectionControl {
                                    state: ConnectionControlState::ConnectResponse(
                                        connection_id.to_string(),
                                        local_addr.port(),
                                        channel_count,
                                    ),
                                };*/
                                send_packet(
                                    &mut socket,
                                    ControlResponse::Stream(connection_id, local_addr.port(), mixer.buffer_size(), mixer.sample_rate()),
                                )
                                .await;
                            }
                        }
                        ControlRequest::CloseStream(uuid) => {
                            if let Some(h) = handles.lock().await.remove(&uuid) {
                                h.stop().await;
                                send_packet(&mut socket, ControlResponse::Ok).await;
                            } else {
                                error!("Couldn't find stream for connection Id {}", uuid);
                                send_packet(
                                    &mut socket,
                                    ControlResponse::Error(ControlError::StreamIdNotFound),
                                )
                                .await;
                            }
                        }
                    }
                }
            });
        }
    }
}
