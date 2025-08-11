use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    task::Poll,
};

use log::{debug, error, info, trace};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{
        Mutex,
        mpsc::{self, Receiver},
    },
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    audio::GLOBAL_MASTER_OUTPUT_MIXER,
    control::{
        packet::{ControlError, ControlRequest, ControlResponse},
    },
    mixer::{MixerTrackSelector, MixerTrait},
    streamer::receiver::AudioReceiverHandle,
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

enum TcpServerCommands {
    Stop,
}

/// struct that represents a tcp server
/// _task holds the task associated with the server
pub struct TcpServer {
    pub _task: JoinHandle<io::Result<()>>,
    handles: Arc<Mutex<HashMap<Uuid, AudioReceiverHandle>>>,
    channel: mpsc::Sender<TcpServerCommands>,
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

        Self {
            _task: tokio::spawn(new_control_server(target_node_addr, handles.clone(), r, on_success)),
            channel: s,
            handles
        }
    }
}

impl Future for TcpServer {
    type Output = io::Result<()>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let task = self.get_mut();

        if task._task.is_finished() {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

//pub async fn new_control_server(sock_addr: String) -> io::Result<()> {}

/// The entry point to the tcp communication server
async fn new_control_server<F, Fut>(
    sock_addr: String,
    handles: Arc<Mutex<HashMap<Uuid, AudioReceiverHandle>>>,
    r: Receiver<TcpServerCommands>,
    on_success: F,
) -> io::Result<()>
where
    F: Fn(MixerTrackSelector) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = io::Result<AudioReceiverHandle>> + Send + 'static,
{
    let ip: Ipv4Addr = sock_addr.parse().expect("parse failed");
    let target = SocketAddr::new(std::net::IpAddr::V4(ip), 6789);
    let listen = TcpListener::bind(target).await?;

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
                let mut buf = vec![0; 8196];
                //let handles = child_handles.lock().await;
                loop {
                    let handles = Arc::clone(&handles);

                    // TODO tcp error handling
                    match read_packet(&mut socket, &mut buf).await.unwrap() {
                        ControlRequest::OpenStream(mixer_track_selector) => {
                            let connection_id = uuid::Uuid::new_v4();

                            // whatever
                            let mixer = GLOBAL_MASTER_OUTPUT_MIXER.lock().await;
                            let mixer = mixer.as_ref().expect("failed to open mixer");

                            let callback = callback.clone();

                            // TODO Implement way for the callback to notify back when its done
                            match (callback)(mixer_track_selector).await {
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
                                    .unwrap();
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
            });
        }
    }
}
