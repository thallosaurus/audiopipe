use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc, task::Poll
};

use log::{debug, error, info, trace};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc::{self, Receiver}, Mutex}, task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    control::packet::{ControlRequest, ControlResponse},
    streamer::sender::UdpClientHandle,
};

async fn send_packet(stream: &mut TcpStream, packet: ControlRequest) -> io::Result<()> {
    debug!("Sending Packet: {:?}", packet);
    let json = serde_json::to_vec(&packet).unwrap();
    trace!("{:?}", json);

    stream.write_all(json.as_slice()).await
}

async fn read_packet(stream: &mut TcpStream, mut buf: &mut [u8]) -> io::Result<ControlResponse> {
    let n = stream.read(&mut buf).await?;

    let data = &buf[..n];
    trace!("{:?}", data);
    let json = serde_json::from_slice(data)?;
    debug!("{:?}", json);
    Ok(json)
}

enum TcpClientCommands {
    Stop,
}

pub struct TcpClient {
    pub _task: JoinHandle<io::Result<()>>,
    channel: mpsc::Sender<TcpClientCommands>,
}

impl TcpClient {
    pub fn new<F, Fut>(
        target_node_addr: String,
        on_success: F, //config: &StreamConfig,
                       //max_buffer_size: u32,
    ) -> Self
    where
        F: Fn(SocketAddr, Uuid, usize, usize) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = UdpClientHandle> + Send + 'static,
    {
        let (s, r) = mpsc::channel(1);

        Self {
            _task: tokio::spawn(tcp_client(target_node_addr, r, on_success)),
            channel: s,
        }
    }
}

impl Future for TcpClient {
    type Output = io::Result<()>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let task = self.get_mut();

        if task._task.is_finished() {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

/// channel count tells us, how many of our available channels we want to send
/// dont get tempted to just pipe in the stream config variable
async fn tcp_client<F, Fut>(
    target_node_addr: String,
    r: Receiver<TcpClientCommands>,
    on_success: F, //config: &StreamConfig,
                   //max_buffer_size: u32,
) -> io::Result<()>
where
    F: Fn(SocketAddr, Uuid, usize, usize) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = UdpClientHandle> + Send + 'static,
{
    let ip: Ipv4Addr = target_node_addr.parse().expect("parse failed");
    let target = SocketAddr::new(std::net::IpAddr::V4(ip), 6789);
    debug!("Connecting to {}", target);

    let mut stream = TcpStream::connect(target).await?;
    info!("Connected to server {}", target);

    // connection packet
    let packet = ControlRequest::OpenStream(crate::mixer::MixerTrackSelector::Stereo(0, 1));
    send_packet(&mut stream, packet).await?;

    // Stores the current udp connection
    let handle: Arc<Mutex<Option<UdpClientHandle>>> = Arc::new(Mutex::new(None));

    // the network message buffer
    // TODO make this better
    let mut buf = vec![0; 1024];

    let callback = Arc::new(on_success);

    loop {
        // buffer reuse
        let json = read_packet(&mut stream, &mut buf).await?;

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

                *handle.lock().await = Some((cb)(target, uuid, bufsize, srate).await);
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
