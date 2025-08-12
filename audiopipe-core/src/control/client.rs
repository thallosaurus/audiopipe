use std::{
    net::{Ipv4Addr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use log::{debug, error, info, trace};
use tokio::{
    io, net::TcpStream, sync::{
        mpsc::{self, UnboundedReceiver}, Mutex
    }, task::{JoinError, JoinHandle}
};
use uuid::Uuid;

use crate::{
    control::packet::{ControlRequest, ControlResponse, PacketError, read_packet, send_packet},
    mixer::MixerTrackSelector,
    streamer::sender::AudioSenderHandle,
};

#[derive(Debug)]
pub enum TcpClientError {
    PacketError(PacketError),
    StreamError(io::Error),
    TcpConnectError(io::Error),
    JoinError(JoinError),
}

#[derive(Debug)]
pub enum TcpClientCommands {
    Stop,
}

pub struct TcpClient {
    pub _task: JoinHandle<io::Result<()>>,
    channel: Option<mpsc::UnboundedSender<TcpClientCommands>>,
    handle: Arc<Mutex<Option<AudioSenderHandle>>>,
}

impl TcpClient {
    pub fn new<F, Fut>(
        target_node_addr: String,
        mixer_track: MixerTrackSelector,
        on_success: F,
    ) -> Self
    where
        F: Fn(SocketAddr, Uuid, usize, usize, MixerTrackSelector) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = AudioSenderHandle> + Send + 'static,
    {
        let (s, r) = mpsc::unbounded_channel();
        let mut c = Self::create(target_node_addr, r, mixer_track, on_success);
        c.channel = Some(s);
        c
    }

    pub fn create<F, Fut>(
        target_node_addr: String,
        mut channel: UnboundedReceiver<TcpClientCommands>,
        mixer_track: MixerTrackSelector,
        on_success: F,
    ) -> Self
    where
        F: Fn(SocketAddr, Uuid, usize, usize, MixerTrackSelector) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = AudioSenderHandle> + Send + 'static,
    {
        // Stores the current udp connection
        let handle: Arc<Mutex<Option<AudioSenderHandle>>> = Arc::new(Mutex::new(None));
        let h = Arc::clone(&handle);

        Self {
            _task: tokio::spawn(async move {
                let h = Arc::clone(&h);
                //let c = Arc::clone(&channel);

                //let mut channel = c.lock().await;
                tokio::select! {
                    c = tcp_client(target_node_addr, h, mixer_track, on_success) => {
                        match c {
                            Ok(_) => {
                                // tcp client exited cleanly
                                trace!("tcp client: exited cleanly");
                            },
                            Err(e) => {
                                // tcp client encountered an error
                                trace!("tcp client: encountered an error {:?}", e);
                            },
                        }
                    }
                    cc = channel.recv() => {
                        match cc {
                            Some(TcpClientCommands::Stop) => {
                                // stop request
                                info!("tcp client stop request");
                                return Ok(())
                            },
                            None => {
                                // error while receiving
                            },
                        }
                    }
                }

                trace!("reached after client state");

                Ok(())
            }),

            handle: Arc::clone(&handle),
            channel: None,
        }
    }

    pub async fn stop(&self) {
        if let Some(h) = self.handle.lock().await.as_ref() {
            h.stop().await.unwrap();
        }

        if let Some(c) = &self.channel {
            c.send(TcpClientCommands::Stop).unwrap();
        }
    }
}

impl Future for TcpClient {
    type Output = Result<(), TcpClientError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let task = self.get_mut();

        match Pin::new(&mut task._task).poll(cx) {
            Poll::Ready(Ok(res)) => Poll::Ready(Ok(())),
            Poll::Ready(Err(res)) => Poll::Ready(Err(TcpClientError::JoinError(res))),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// channel count tells us, how many of our available channels we want to send
/// dont get tempted to just pipe in the stream config variable
async fn tcp_client<F, Fut>(
    target_node_addr: String,
    handle: Arc<Mutex<Option<AudioSenderHandle>>>,
    sel: MixerTrackSelector,
    on_success: F
) -> Result<(), TcpClientError>
where
    F: Fn(SocketAddr, Uuid, usize, usize, MixerTrackSelector) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = AudioSenderHandle> + Send + 'static,
{
    let ip: Ipv4Addr = target_node_addr.parse().expect("parse failed");
    let target = SocketAddr::new(std::net::IpAddr::V4(ip), 6789);
    debug!("Connecting to {}", target);

    let mut stream = TcpStream::connect(target)
        .await
        .map_err(|e| TcpClientError::StreamError(e))?;
    //.map_err(|e|TcpClientError::TcpConnectError(e))?;
    info!("Connected to server {}", target);

    // connection packet
    let packet = ControlRequest::OpenStream(crate::mixer::MixerTrackSelector::Stereo(0, 1));
    send_packet(&mut stream, packet)
        .await
        .map_err(|e| TcpClientError::PacketError(e))?;
    //.map_err(|e|TcpClientError::StreamSendError(e))?;

    // the network message buffer
    // TODO make this better
    let mut buf = vec![0; 1024];

    let callback = Arc::new(on_success);

    loop {
        // buffer reuse
        let json = read_packet(&mut stream, &mut buf).await.map_err(|e|TcpClientError::PacketError(e))?;
        //.map_err(|e|TcpClientError::StreamReadError(e))?;

        debug!("< Read Packet: {:?}", json);

        let handle = handle.clone();

        match json {
            ControlResponse::Stream(uuid, port, bufsize, srate) => {
                let ip: Ipv4Addr = target_node_addr.parse().expect("parse failed");
                let target = SocketAddr::new(std::net::IpAddr::V4(ip), port);
                info!(
                    "Connection to peer {} with connection id {} successful",
                    target, uuid
                );

                //*handle.lock().await = Some(UdpClientHandle::start_audio_stream_client(target, uuid, bufsize, srate).await);

                let cb = callback.clone();

                *handle.lock().await = Some((cb)(target, uuid, bufsize, srate, sel).await);
            }
            ControlResponse::Ok => todo!(),
            ControlResponse::Error(control_error) => {
                error!("{}", control_error);
                break;
            }
        }
    }
    Ok(())
}
