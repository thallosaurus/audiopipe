use std::{
    net::SocketAddr,
    time::{Duration, SystemTime},
};

use log::info;
use tokio::{
    io,
    net::UdpSocket,
    sync::mpsc::{self, error::SendError},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    audio::GLOBAL_MASTER_INPUT_MIXER,
    streamer::packet::{AudioPacket, AudioPacketHeader},
};

pub enum UdpClientCommands {
    Stop,
}

pub struct UdpClientHandle {
    //_handle: Pin<Box<dyn Future<Output = io::Result<()>> + Send>>,
    _handle: JoinHandle<io::Result<()>>,
    channel: mpsc::Sender<UdpClientCommands>,
    connection_id: Uuid,
}

impl UdpClientHandle {
    pub async fn start_audio_stream_client(
        addr: SocketAddr,
        //smprt: u32,
        //chcount: usize,
        connection_id: Uuid,
        bufsize: usize,
        sample_rate: usize,
    ) -> UdpClientHandle {
        let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();

        info!("Connecting UDP to {}", addr);
        sock.connect(addr).await.unwrap();

        let (s, r) = mpsc::channel(1);

        //info!("Starting UDP Sender");
        UdpClientHandle {
            //_handle: Box::pin(udp_client(sock, bufsize * chcount as u32, r)),
            _handle: tokio::spawn(udp_client(sock, r, bufsize, sample_rate, connection_id)),
            channel: s,
            connection_id,
        }
    }
    pub async fn stop(&self) -> Result<(), SendError<UdpClientCommands>> {
        self.channel.send(UdpClientCommands::Stop).await
    }
}

pub async fn udp_client(
    sock: UdpSocket,
    mut ch: mpsc::Receiver<UdpClientCommands>,
    bufsize: usize, //    track: MixerTrack<AsyncRawMixerTrack<Output>>,
    sample_rate: usize,
    connection_id: Uuid,
) -> io::Result<()> {
    info!("Starting UDP Sender");

    // set udp network buffer to the buffersize determined by the server
    let buf: Vec<f32> = vec![0.0f32; bufsize];

    let ms = (bufsize as f64 / sample_rate as f64) * 1000.0;

    // TODO move this to channel selector somehow
    let channels = 2;

    loop {
        let input = GLOBAL_MASTER_INPUT_MIXER.lock().await;

        let input = input.as_ref().expect("failed to open mixer");

        tokio::select! {
        Some(cmd) = ch.recv() => {
            match cmd {
                UdpClientCommands::Stop => break,
            }
        },

        // fixed-interval push
            _ = tokio::time::sleep(Duration::from_millis(ms as u64)) => {
                //TODO Collect data

                let payload: &[u8] = bytemuck::try_cast_slice(&buf).unwrap();

                let packet = AudioPacket {
                    header: AudioPacketHeader {
                        connection_id,
                        timestamp: SystemTime::now(),
                        sample_rate,
                        channels
                    },
                    payload: Vec::from(payload)
                };

                let data = bincode2::serialize(&packet).expect("error while serializing audio data");
                sock.send(&data).await.unwrap();
            }
        }
    }

    Ok(())
}

//TODO supply source channel selector
/*async fn collect_audio_samples(channel: &AsyncMixerOutputEnd) -> (usize, usize) {
    Some(())
}*/
