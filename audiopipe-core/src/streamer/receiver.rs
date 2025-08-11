use std::{fmt::{Display, Write}, net::SocketAddr, sync::Arc};

use log::{debug, error, info, trace};
use ringbuf::traits::Producer;
use tokio::{
    io,
    net::UdpSocket,
    sync::mpsc::{self, error::SendError},
    task::JoinHandle,
};

use crate::{
    audio::GLOBAL_MASTER_OUTPUT_MIXER, mixer::{write_to_mixer_async, AsyncMixerInputEnd, AsyncRawMixerTrack, Input, MixerTrack, MixerTrackSelector, MixerTrait, Output}, streamer::packet::{AudioPacket, TokioUdpAudioPacket}
};

pub enum UdpServerCommands {
    Stop,
}

pub enum UdpServerHandleError {
    TokioError(io::Error)
}

impl Display for UdpServerHandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UdpServerHandleError::TokioError(error) => f.write_fmt(format_args!("tokio error: {}", error)),
        }
    }
}

pub type ReceiverResult<T> = Result<T, UdpServerHandleError>;

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
        //ch: &AsyncMixerInputEnd,
        sel: MixerTrackSelector
    ) -> ReceiverResult<UdpServerHandle> {
        let sock = UdpSocket::bind("0.0.0.0:0").await.map_err(|e|UdpServerHandleError::TokioError(e))?;
        let local_addr = sock.local_addr().map_err(|e|UdpServerHandleError::TokioError(e))?;
        info!("local udp addr: {}", local_addr);

        let (s, r) = mpsc::channel(1);

        Ok(UdpServerHandle {
            //_handle: Arc::new(Mutex::new(Box::pin(udp_server(sock, r, ch)))), //info!("UDP Server Listening");
            _handle: tokio::spawn(udp_server(sock, r, sel)),
            channel: s,
            local_addr,
        })
    }
}

pub async fn udp_server(
    sock: UdpSocket,
    mut ch: mpsc::Receiver<UdpServerCommands>,
    //mixer: SharedInputMixer,
    //raw_ch: &AsyncMixerInputEnd,
    stream_track_selector: MixerTrackSelector
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

                                                    // whatever
                        let mixer = GLOBAL_MASTER_OUTPUT_MIXER.lock().await;
                        let mixer = mixer.as_ref().expect("failed to open mixer");
                        
                        trace!("UDP Packet Length: {:?}", len);
                        let packet: AudioPacket = bincode2::deserialize(&mut buf[..len]).unwrap();
                        //.map_err(|e| UdpError::DeserializeError(e)).unwrap();
                        trace!("Received Packet from {}, Length: {}, {:?}", addr, len, packet);
                        
                        let data: &[f32] = bytemuck::try_cast_slice(&packet.payload).unwrap();

                        write_to_mixer_async(mixer, data, stream_track_selector).await;


                            
                        // TODO Get Output Channel for server
                        //insert_audio_samples(packet, &raw_ch).await;

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