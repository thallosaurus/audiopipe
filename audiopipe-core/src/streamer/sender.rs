use std::{
    net::SocketAddr,
    time::{Duration, SystemTime},
};

use bytemuck::PodCastError;
use log::{error, info};
use tokio::{
    io,
    net::UdpSocket,
    sync::mpsc::{self, error::SendError},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    audio::GLOBAL_MASTER_INPUT_MIXER,
    mixer::{
        MixerError, MixerTrackSelector, MixerTrait, read_from_mixer_async,
        read_from_mixer_track_async,
    },
    streamer::packet::{AudioPacket, AudioPacketHeader},
};

pub enum UdpClientCommands {
    Stop,
}

#[derive(Debug)]
pub enum UdpClientError {
    MixerError(MixerError),
    CastError(PodCastError)
}

pub struct AudioSenderHandle {
    //_handle: Pin<Box<dyn Future<Output = io::Result<()>> + Send>>,
    _handle: JoinHandle<io::Result<()>>,
    channel: mpsc::Sender<UdpClientCommands>,
    connection_id: Uuid,
}

impl AudioSenderHandle {
    pub async fn new(
        addr: SocketAddr,
        //smprt: u32,
        //chcount: usize,
        connection_id: Uuid,
        bufsize: usize,
        sample_rate: usize,
        track_selector: MixerTrackSelector,
    ) -> AudioSenderHandle {
        let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();

        info!("Connecting UDP to {}", addr);
        sock.connect(addr).await.unwrap();

        let (s, r) = mpsc::channel(1);

        //info!("Starting UDP Sender");
        AudioSenderHandle {
            //_handle: Box::pin(udp_client(sock, bufsize * chcount as u32, r)),
            _handle: tokio::spawn(async move {
                match udp_client(sock, r, bufsize, sample_rate, connection_id, track_selector).await {
                    Ok(_) => {
                        return Ok(())
                    },
                    Err(e) => {
                        error!("encountered error: {:?}", e);
                    },
                }
                Ok(())
            }),
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
    sel: MixerTrackSelector,
) -> Result<(), UdpClientError> {
    info!("Starting UDP Sender");

    // set udp network buffer to the buffersize determined by the server
    let mut buf: Vec<f32> = vec![0.0f32; bufsize * sel.channel_count()];

    let ms = (bufsize as f64 / sample_rate as f64) * 1000.0;

    // TODO move this to channel selector somehow
    //let channels = 2;

    loop {
        let input = GLOBAL_MASTER_INPUT_MIXER.lock().await;

        let input = input.as_ref().expect("failed to open mixer");

        tokio::time::sleep(Duration::from_millis(ms as u64)).await;
        //let (consumed, dropped) = read_from_mixer_async(input, &mut buf).await.unwrap();

        let channels = input
            .get_channel(sel)
            .map_err(|e| UdpClientError::MixerError(e))?;
        read_from_mixer_track_async(&channels, &mut buf).await;

        let payload: &[u8] = bytemuck::try_cast_slice(&buf).map_err(|e|UdpClientError::CastError(e))?;

        let packet = AudioPacket {
            header: AudioPacketHeader {
                connection_id,
                timestamp: SystemTime::now(),
                sample_rate,
                channels: sel.channel_count(),
            },
            payload: Vec::from(payload),
        };

        let data = bincode2::serialize(&packet).expect("error while serializing audio data");
        sock.send(&data).await.unwrap();
    }

    Ok(())
}

//TODO supply source channel selector
/*async fn collect_audio_samples(channel: &AsyncMixerOutputEnd) -> (usize, usize) {
    Some(())
}*/

#[cfg(test)]
pub(crate) mod tests {
    use std::{net::SocketAddr, time::Duration};

    use tokio::sync::mpsc::{self};
    use uuid::Uuid;

    use crate::{mixer::MixerTrackSelector, streamer::sender::AudioSenderHandle};

    pub async fn dummy_sender(
        addr: SocketAddr,
        //smprt: u32,
        //chcount: usize,
        connection_id: Uuid,
        bufsize: usize,
        sample_rate: usize,
        sel: MixerTrackSelector,
    ) -> AudioSenderHandle {
        let (s, r) = mpsc::channel(1);
        AudioSenderHandle {
            _handle: tokio::spawn(async move {
                log::debug!("dummy connection to {}", addr);
                assert!(true);
                //loop {
                //tokio::time::sleep(Duration::from_millis(10000)).await;
                //}
                Ok(())
            }),
            channel: s,
            connection_id,
        }
    }

    #[tokio::test]
    async fn test_audio_sender_stop() {
        let addr: SocketAddr = "127.0.0.1:4000".parse().unwrap();
        let connection_id = Uuid::new_v4();

        let sender =
            AudioSenderHandle::new(addr, connection_id, 256, 44100, MixerTrackSelector::Mono(0))
                .await;

        let res = sender.stop().await;
        assert!(res.is_ok());
    }
}
