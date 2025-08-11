use std::{
    collections::HashMap,
    fmt::Debug,
    net::{Ipv4Addr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use log::{debug, error, info, trace};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver}, Mutex
    },
    task::{JoinError, JoinHandle},
};
use uuid::Uuid;

type SharedAudioReceiverHandle = Arc<Mutex<HashMap<uuid::Uuid, AudioReceiverHandle>>>;

use crate::{
    audio::GLOBAL_MASTER_OUTPUT_MIXER,
    control::packet::{
        ControlError, ControlRequest, ControlResponse, PacketError, read_packet, send_packet,
    },
    mixer::{MixerTrackSelector, MixerTrait},
    streamer::receiver::AudioReceiverHandle,
};

enum TcpServerCommands {
    Stop,
}

#[derive(Debug)]
pub enum TcpServerErrors {
    ClientDisconnectEof,
    SocketError(io::Error),
    JoinError(JoinError)
}

#[derive(Debug)]
pub enum TcpServerHandlerErrors {
    //SocketError(io::Error),
    HandlerPacketError(PacketError),
    SerdeError(serde_json::Error),
}

/// struct that represents a tcp server
pub struct TcpServer {
    /// Holds the associated task handle
    pub _task: JoinHandle<()>,

    /// Holds all active Audio Stream Handles
    handles: SharedAudioReceiverHandle,
    // MPSC Channel for program control
    //channel: mpsc::Sender<TcpServerCommands>,
}

impl TcpServer {
    pub fn new<F, Fut>(target_node_addr: String, on_success: F) -> Self
    where
        F: Fn(MixerTrackSelector) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = io::Result<AudioReceiverHandle>> + Send + 'static,
    {
        let (s, r) = mpsc::channel(1);

        // holds all open udp audio streams
        let handles = Arc::new(Mutex::new(HashMap::new()));

        let h = Arc::clone(&handles);

        Self {
            _task: tokio::spawn(async move {
                assert!(true);
                match server_event_loop(target_node_addr, h, r, on_success).await {
                    Ok(_) => {
                        // event loop exited cleanly
                        debug!("event loop exited cleanly");
                    }
                    Err(e) => {}
                }
            }),
            //channel: s,
            handles,
        }
    }
}

impl Future for TcpServer {
    type Output = Result<(), TcpServerErrors>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut task = self.get_mut();

        match Pin::new(&mut task._task).poll(cx) {
            Poll::Ready(Ok(res)) => {
                Poll::Ready(Ok(()))
            },
            Poll::Ready(Err(res)) => {
                Poll::Ready(Err(TcpServerErrors::JoinError(res)))
            }
            Poll::Pending => todo!(),
        }
    }
}

//pub async fn new_control_server(sock_addr: String) -> io::Result<()> {}

/// The entry point to the tcp communication server
async fn server_event_loop<F, Fut>(
    sock_addr: String,
    handles: Arc<Mutex<HashMap<Uuid, AudioReceiverHandle>>>,
    r: Receiver<TcpServerCommands>,
    on_success: F,
) -> Result<(), TcpServerErrors>
where
    F: Fn(MixerTrackSelector) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = io::Result<AudioReceiverHandle>> + Send + 'static,
{
    let ip: Ipv4Addr = sock_addr.parse().expect("parse failed");
    let target = SocketAddr::new(std::net::IpAddr::V4(ip), 6789);
    let listen = TcpListener::bind(target)
        .await
        .map_err(|e| TcpServerErrors::SocketError(e))?;

    //let rc = Arc::new(Mutex::new(r));
    let callback = Arc::new(on_success);

    info!("Server Listening");
    loop {
        if let Ok((mut socket, _client_addr)) = listen.accept().await {
            // copy handle for udp streams
            let handles = Arc::clone(&handles);

            let callback = callback.clone();

            // spawn a new task for the connection
            tokio::spawn(async move {
                // handle client connections
                match handle_connection(&mut socket, handles, callback).await {
                    Ok(_) => {
                        // handle exited cleanly
                        debug!("handle stopped cleanly");
                    }
                    Err(e) => {
                        // an error occurred on the connection
                        debug!("handle error: {:?}", e);
                    }
                }
            });
        }
    }
}

async fn handle_connection<F, Fut>(
    mut socket: &mut TcpStream,
    handles: SharedAudioReceiverHandle,
    on_success: Arc<F>,
) -> Result<(), TcpServerHandlerErrors>
where
    F: Fn(MixerTrackSelector) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = io::Result<AudioReceiverHandle>> + Send + 'static,
{
    let mut buf = vec![0; 8196];
    //let handles = child_handles.lock().await;
    loop {
        let handles = Arc::clone(&handles);

        // TODO tcp error handling
        let packet = read_packet(&mut socket, &mut buf)
            .await
            .map_err(|e| TcpServerHandlerErrors::HandlerPacketError(e))?;
        match packet {
            ControlRequest::OpenStream(mixer_track_selector) => {
                let connection_id = uuid::Uuid::new_v4();

                let mixer = GLOBAL_MASTER_OUTPUT_MIXER.lock().await;
                let mixer = mixer.as_ref().expect("failed to open mixer");

                let callback = on_success.clone();

                let cb_ret = (callback)(mixer_track_selector).await;
                match cb_ret {
                    Ok(handle) => {
                        let local_addr = handle.local_addr.clone();
                        handles.lock().await.insert(connection_id, handle);
                        
                        info!("new udp connection id {}", connection_id);
                        send_packet(
                            &mut socket,
                            ControlResponse::Stream(
                                connection_id,
                                local_addr.port(),
                                mixer.buffer_size(),
                                mixer.sample_rate(),
                            ),
                        )
                        .await
                        // TODO Implement way for the callback to notify back when its done
                        .map_err(|e| TcpServerHandlerErrors::HandlerPacketError(e))?

                    }
                    Err(err) => {
                        error!("{}", err);
                    }
                }
            }
            ControlRequest::CloseStream(uuid) => {
                if let Some(h) = handles.lock().await.remove(&uuid) {
                    h.stop().await.unwrap();
                    send_packet(&mut socket, ControlResponse::Ok).await.unwrap();
                } else {
                    error!("Couldn't find stream for connection Id {}", uuid);
                    send_packet(
                        &mut socket,
                        ControlResponse::Error(ControlError::StreamIdNotFound),
                    )
                    .await
                    .unwrap();
                }
            }
        }
    }
}
