use std::net::SocketAddr;

use log::{debug, error, info, trace};
use ringbuf::traits::Producer;
use tokio::{
    io,
    net::UdpSocket,
    sync::mpsc::{self, error::SendError},
    task::JoinHandle,
};

use crate::{
    mixer::{AsyncRawMixerTrack, Input, MixerTrack}, streamer::packet::TokioUdpAudioPacket,
};

pub enum UdpServerCommands {
    Stop,
}

pub struct UdpServerHandle {
    //_handle: UdpServerHandleFuture,
    _handle: JoinHandle<io::Result<()>>,
    channel: mpsc::Sender<UdpServerCommands>,
    pub local_addr: SocketAddr,
}

impl UdpServerHandle {
    pub async fn stop(&self) -> Result<(), SendError<UdpServerCommands>> {
        self.channel.send(UdpServerCommands::Stop).await
    }

    pub async fn start_audio_stream_server(
        ch: MixerTrack<AsyncRawMixerTrack<Input>>,
    ) -> UdpServerHandle {
        let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        let local_addr = sock.local_addr().unwrap();
        info!("local udp addr: {}", local_addr);

        let (s, r) = mpsc::channel(1);

        //let mixer_clone = mixer.clone();
        UdpServerHandle {
            //_handle: Arc::new(Mutex::new(Box::pin(udp_server(sock, r, ch)))), //info!("UDP Server Listening");
            _handle: tokio::spawn(udp_server(sock, r, ch)),
            channel: s,
            local_addr,
        }
    }
}

pub async fn udp_server(
    sock: UdpSocket,
    mut ch: mpsc::Receiver<UdpServerCommands>,
    //mixer: SharedInputMixer,
    raw_ch: MixerTrack<AsyncRawMixerTrack<Input>>,
) -> io::Result<()> {
    info!("Starting UDP Receiver");
    // start connection
    let mut buf = vec![0; 10000 as usize].into_boxed_slice();

    loop {
        tokio::select! {
            result = sock.recv_from(&mut buf) => {
                match result {
                    Ok((len, addr)) => {
                        //info!("Received {:?} bytes from {:?}, Payload: {:?}", len, addr, data);

                        trace!("UDP Packet Length: {:?}", len);
                        let packet: TokioUdpAudioPacket = bincode2::deserialize(&mut buf[..len]).unwrap();
                            //.map_err(|e| UdpError::DeserializeError(e)).unwrap();
                        trace!("Received Packet from {}, Length: {}, {:?}", addr, len, packet);

                        // TODO Get Output Channel for server
                        insert_audio_samples(packet, &raw_ch).await;

                        // decode packet
                        //info!("{:?}", String::from_utf8(data.to_vec()));
                    },
                    Err(e) => {
                        error!("recv error: {:?}", e);
                    }
                }
            },

            Some(cmd) = ch.recv() => {
                match cmd {
                    UdpServerCommands::Stop => break,
                }
            },
        };
    }

    Ok(())
}

/// Function that gets called when the udp socket received a new AudioPacket
///
/// Writes to the master mixer
async fn insert_audio_samples(
    packet: TokioUdpAudioPacket,
    channel: &MixerTrack<AsyncRawMixerTrack<Input>>,
) {
    // Convert the buffered network samples to the specified sample format
    let converted_samples: &[f32] = bytemuck::try_cast_slice(&packet.data).unwrap();
    //.map_err(|e| UdpError::CastingError(e))?;
    //let mut output = GLOBAL_MASTER_OUTPUT.lock().await;

    let mut dropped = 0;
    let mut consumed = 0;

    match channel {
        MixerTrack::Mono(ch) => todo!(),
        MixerTrack::Stereo(l, r) => {
            let mut b = 0;

            //let mut mixer = mixer.lock().await;
            //mixer.get_channel(channel)

            for &sample in converted_samples {
                // TODO implement fell-behind logic here
                let ch_selector = b % packet.channels.len();

                let mut c;
                if ch_selector == 0 {
                    c = l.lock().await;
                } else {
                    c = r.lock().await;
                }

                if let Err(err) = c.try_push(sample) {
                    dropped += 1;
                } else {
                    consumed += 1;
                }

                b += 1;
            }
        }
    }
    debug!("{} consumed, {} dropped", consumed, dropped);
}
