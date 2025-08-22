use std::{
    collections::HashMap,
    fmt::Debug,
    io::Error,
    net::{Ipv4Addr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use log::{debug, error, info};
use tokio::{
    io::{self},
    net::{TcpListener, TcpStream},
    sync::{
        Mutex,
        mpsc::{self, Receiver, UnboundedReceiver},
    },
    task::{JoinError, JoinHandle},
};
use uuid::Uuid;

type SharedAudioReceiverHandle = Arc<Mutex<HashMap<uuid::Uuid, AudioReceiverHandle>>>;

use crate::{
    audio::GLOBAL_MASTER_OUTPUT_MIXER,
    control::packet::{
        read_packet, send_packet, ControlError, ControlRequest, ControlResponse, PacketError
    },
    mixer::{MixerTrackSelector, MixerTrait},
    streamer::receiver::{AudioReceiverHandle, UdpServerHandleError},
};

enum TcpServerCommands {
    Stop,
}

#[derive(Debug)]
pub enum TcpServerErrors {
    ClientDisconnectEof,
    SocketError(io::Error),
    JoinError(JoinError),
}

#[derive(Debug)]
pub enum TcpServerHandlerErrors {
    //SocketError(io::Error),
    HandlerPacketError(PacketError),
    SerdeError(serde_json::Error),
    AudioStreamError(UdpServerHandleError),
    StreamClosed(Option<Uuid>),
}

/// struct that represents a tcp server
pub struct TcpServer {
    /// Holds the associated task handle
    pub _task: JoinHandle<()>,

    /// Holds all active Audio Stream Handles
    handles: SharedAudioReceiverHandle,
    // MPSC Channel for program control
    channel: Option<mpsc::UnboundedSender<TcpServerCommands>>,
}

impl TcpServer {
    pub fn new<F, Fut>(target_node_addr: String, on_success: F) -> Self
    where
        F: Fn(MixerTrackSelector) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<AudioReceiverHandle, UdpServerHandleError>> + Send + 'static,
    {
        let (s, r) = mpsc::unbounded_channel();
        let mut server = Self::create(target_node_addr, r, on_success);
        server.channel = Some(s);
        server
    }
    fn create<F, Fut>(
        target_node_addr: String,
        mut channel: UnboundedReceiver<TcpServerCommands>,
        on_success: F,
    ) -> Self
    where
        F: Fn(MixerTrackSelector) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<AudioReceiverHandle, UdpServerHandleError>> + Send + 'static,
    {
        // holds all open udp audio streams
        let handles = Arc::new(Mutex::new(HashMap::new()));

        let h = Arc::clone(&handles);

        Self {
            _task: tokio::spawn(async move {
                assert!(true);
                let ip: Ipv4Addr = target_node_addr.parse().expect("parse failed");
                let target = SocketAddr::new(std::net::IpAddr::V4(ip), 6789);
                if let Ok(listen) = TcpListener::bind(target).await {
                    //.map_err(|e| TcpServerErrors::SocketError(e))?;
                    match server_event_loop(listen, h, channel, on_success).await {
                        Ok(_) => {
                            // event loop exited cleanly
                            debug!("event loop exited cleanly");
                        }
                        Err(e) => {
                            error!("event loop encounted an error: {:?}", e);
                        }
                    }
                } else {
                    // couldn't open TcpListener
                }
            }),
            channel: None,
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
        let task = self.get_mut();

        match Pin::new(&mut task._task).poll(cx) {
            Poll::Ready(Ok(res)) => Poll::Ready(Ok(res)),
            Poll::Ready(Err(res)) => Poll::Ready(Err(TcpServerErrors::JoinError(res))),
            Poll::Pending => Poll::Pending,
        }
    }
}

//pub async fn new_control_server(sock_addr: String) -> io::Result<()> {}

/// The entry point to the tcp communication server
async fn server_event_loop<F, Fut>(
    listen: TcpListener,
    //sock_addr: String,
    handles: Arc<Mutex<HashMap<Uuid, AudioReceiverHandle>>>,
    channel: UnboundedReceiver<TcpServerCommands>,
    on_success: F,
) -> Result<(), TcpServerErrors>
where
    F: Fn(MixerTrackSelector) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<AudioReceiverHandle, UdpServerHandleError>> + Send + 'static,
{
    /*let ip: Ipv4Addr = sock_addr.parse().expect("parse failed");
    let target = SocketAddr::new(std::net::IpAddr::V4(ip), 6789);
    let listen = TcpListener::bind(target)
        .await
        .map_err(|e| TcpServerErrors::SocketError(e))?;*/

    //let rc = Arc::new(Mutex::new(r));
    let callback = Arc::new(on_success);
    let ch = Arc::new(Mutex::new(channel));

    let mut cc = ch.lock().await;
    info!("Server Listening");
    loop {
        tokio::select! {
            c = cc.recv() => {
                // channel has received a command
            },
            res = listen.accept() => {
                match res {
                    Ok((mut socket, _client_addr)) => {
                        // copy handle for udp streams
                        let handles = Arc::clone(&handles);

                        let callback = callback.clone();

                        // spawn a new task for the connection
                        tokio::spawn(async move {
                            let h = Arc::clone(&handles);
                            // handle client connections
                            match handle_connection(&mut socket, handles.clone(), callback).await {
                                Ok(_) => {
                                    // handle exited cleanly
                                    debug!("handle stopped cleanly");
                                },
                                Err(e) => {
                                    debug!("server event loop encountered an error: {:?}", e);
                                    // an error occurred on the connection
                                    match e {
                                        TcpServerHandlerErrors::HandlerPacketError(packet_error) => {
                                            error!("packet error: {:?}", packet_error);
                                            let h = handles.lock().await;
                                            debug!("active handlers: {}", h.len());
                                            return;
                                        }
                                        TcpServerHandlerErrors::SerdeError(error) => todo!(),
                                        TcpServerHandlerErrors::AudioStreamError(error) => todo!(),
                                        TcpServerHandlerErrors::StreamClosed(Some(uuid)) => {
                                            let _ = remove_handle(h, &uuid).await;
                                            let h = handles.lock().await;
                                            debug!("active handlers: {}", h.len());
                                        }
                                        _ => {
                                            // do nothing
                                        }
                                    }
                                }
                            }
                        });
                    },
                    Err(e) => {

                    }
                }
            }
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
    Fut: Future<Output = Result<AudioReceiverHandle, UdpServerHandleError>> + Send + 'static,
{
    let mut packet_buffer = vec![0; 8196];
    //let handles = child_handles.lock().await;
    let current_id: Arc<Mutex<Option<Uuid>>> = Arc::new(Mutex::new(None));
    loop {
        let handles = Arc::clone(&handles);

        let packet = read_packet(&mut socket, &mut packet_buffer).await;

        match packet {
            Ok(ControlRequest::OpenStream(mixer_track_selector)) => {
                let connection_id = uuid::Uuid::new_v4();

                let mixer = GLOBAL_MASTER_OUTPUT_MIXER.lock().await;
                let mixer = mixer.as_ref().expect("failed to open mixer");

                let callback = on_success.clone();

                let h = (callback)(mixer_track_selector)
                    .await
                    .map_err(|e| TcpServerHandlerErrors::AudioStreamError(e))?;

                let local_addr = h.local_addr.clone();
                handles.lock().await.insert(connection_id, h);

                info!("new udp connection id {}", connection_id);
                *current_id.lock().await = Some(connection_id);

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
                .map_err(|e| TcpServerHandlerErrors::HandlerPacketError(e))?;
            }
            Ok(ControlRequest::CloseStream(uuid)) => {
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
            Err(e) => {
                return Err(TcpServerHandlerErrors::StreamClosed(
                    *current_id.lock().await,
                ));
            }
        }
    }
}

async fn remove_handle(handles: SharedAudioReceiverHandle, id: &Uuid) -> Result<(), ControlError> {
    if let Some(h) = handles.lock().await.remove(id) {
        //h.stop().await.unwrap();
        Ok(())
    } else {
        Err(ControlError::StreamIdNotFound)
    }
}
