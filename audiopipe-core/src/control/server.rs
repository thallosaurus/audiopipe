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

use crate::{
    audio::GLOBAL_MASTER_OUTPUT_MIXER,
    control::{
        packet::{
            ControlError, ControlRequest, ControlResponse, PacketError, read_packet, send_packet,
        },

    },
    mixer::{MixerTrackSelector, MixerTrait},
    streamer::receiver::{AudioReceiverHandle, UdpServerHandleError},
};

type SharedAudioReceiverHandle = Arc<Mutex<HashMap<uuid::Uuid, AudioReceiverHandle>>>;

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
    CleanExit,
}

/// struct that represents a tcp server
pub struct TcpServer {
    /// Holds the associated task handle
    pub _task: Box<JoinHandle<Result<(), TcpServerHandlerErrors>>>,

    /// Holds all active Audio Stream Handles
    handles: SharedAudioReceiverHandle,
    // MPSC Channel for program control
    channel: Option<mpsc::UnboundedSender<TcpServerCommands>>,
}

impl TcpServer {
    /// Creates a new TcpServer
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

    /// Raw Function that creates a tokio task that implements the tcp server. Use [TcpServer::new] instead
    ///
    /// Creates a new hashmap for all the handles, spawns a boxed tokio task with the given channel, so that we can control the tokio task from outside,
    /// see [TcpServerCommands] for all available commands
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
        let cb = Arc::new(on_success);

        let ch = Arc::new(Mutex::new(channel));

        let handles_clone = handles.clone();
        Self {
            _task: Box::new(tokio::spawn(async move {
                assert!(true);

                // parse server ip address
                let ip: Ipv4Addr = target_node_addr.parse().expect("parse failed");
                let target = SocketAddr::new(std::net::IpAddr::V4(ip), 6789);
                let h = Arc::clone(&handles);
                if let Ok(listen) = TcpListener::bind(target).await {
                    loop {
                        let h = Arc::clone(&handles);
                        let mut cc_clone = ch.clone();
                        let mut cc = ch.lock().await;
                        tokio::select! {
                            c = cc.recv() => {
                            // channel has received a command
                                debug!("channel has received a command");
                                break
                            },
                            res = listen.accept() => {
                                match res {
                                    Ok((mut socket, _)) => {
                                        let callback = Arc::clone(&cb);
                                        //Self::event_loop(socket, h, cc_clone, callback).await;
                                        // We have a new connection!
                                        tokio::spawn(async move {
                                            match Self::handle_connection(&mut socket, h, callback).await {
                                                Ok(_) => {

                                                },
                                                Err(e) => todo!(),
                                            }
                                        });
                                    },
                                    Err(e) => {

                                    }
                                }
                            }
                        }
                    }

                    //.map_err(|e| TcpServerErrors::SocketError(e))?;
                    /*match Self::event_loop(listen, h, channel, on_success).await {
                        Ok(_) => {
                            // event loop exited cleanly
                            debug!("event loop exited cleanly");
                            return Err(TcpServerHandlerErrors::CleanExit);
                        }
                        Err(e) => {
                            error!("event loop encounted an error: {:?}", e);
                            return Err(e);
                        }
                    }*/
                } else {
                    // couldn't open TcpListener
                }
                Ok(())
            })),
            //)),
            channel: None,
            handles: handles_clone,
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

    pub async fn stop(&self) -> Result<(), TcpServerErrors> {
        let ch = self.channel.clone().unwrap();
        _ = ch.send(TcpServerCommands::Stop);
        Ok(())
    }

    pub fn wait_for_stop(&self) {
        while !self._task.is_finished() {
            // wait until task is closed
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
            Poll::Ready(Ok(res)) => Poll::Ready(Ok(res.expect("what the fuck"))),
            Poll::Ready(Err(res)) => Poll::Ready(Err(TcpServerErrors::JoinError(res))),
            Poll::Pending => Poll::Pending,
        }
    }
}

//pub async fn new_control_server(sock_addr: String) -> io::Result<()> {}

async fn remove_handle(handles: SharedAudioReceiverHandle, id: &Uuid) -> Result<(), ControlError> {
    if let Some(h) = handles.lock().await.remove(id) {
        //h.stop().await.unwrap();
        Ok(())
    } else {
        Err(ControlError::StreamIdNotFound)
    }
}
