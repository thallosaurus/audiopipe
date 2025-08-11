use std::{fmt::Display, net::SocketAddr, pin::Pin, task::Poll};

use log::{error, info, trace};
use serde::{Deserialize, Serialize};
use tokio::{
    io,
    net::UdpSocket,
    sync::mpsc::{self, error::SendError},
    task::JoinHandle,
};

use crate::{
    audio::GLOBAL_MASTER_OUTPUT_MIXER,
    mixer::{MixerTrackSelector, write_to_mixer_async},
    streamer::packet::AudioPacket,
};

pub enum UdpServerCommands {
    Stop,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum UdpServerHandleError {
    TokioError(String),
    CastingError,
    SerializationError,
}

impl Display for UdpServerHandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UdpServerHandleError::TokioError(error) => {
                f.write_fmt(format_args!("tokio error: {}", error))
            }
            UdpServerHandleError::CastingError => f.write_fmt(format_args!("casting error")),
            UdpServerHandleError::SerializationError => {
                f.write_fmt(format_args!("serialization error"))
            }
        }
    }
}

pub type ReceiverResult<T> = Result<T, UdpServerHandleError>;

pub struct AudioReceiverHandle {
    //_handle: UdpServerHandleFuture,
    _handle: JoinHandle<Result<(), UdpServerHandleError>>,
    channel: mpsc::Sender<UdpServerCommands>,
    pub local_addr: SocketAddr,
}

impl AudioReceiverHandle {
    pub async fn stop(&self) -> Result<(), SendError<UdpServerCommands>> {
        self.channel.send(UdpServerCommands::Stop).await
    }

    pub async fn new(
        //ch: &AsyncMixerInputEnd,
        sel: MixerTrackSelector,
    ) -> io::Result<AudioReceiverHandle> {
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        //.map_err(|e| UdpServerHandleError::TokioError(e.to_string()))?;
        let local_addr = sock.local_addr()?;
        //.map_err(|e| UdpServerHandleError::TokioError(e.to_string()))?;
        info!("local udp addr: {}", local_addr);

        let (s, r) = mpsc::channel(1);
        Ok(AudioReceiverHandle {
            //_handle: Arc::new(Mutex::new(Box::pin(udp_server(sock, r, ch)))), //info!("UDP Server Listening");
            _handle: tokio::spawn(async move {
                udp_server(sock, r, sel).await
            }),
            channel: s,
            local_addr,
        })
    }
}

impl Future for AudioReceiverHandle {
    type Output = Result<(), UdpServerHandleError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {

        let task = self.get_mut();

        match Pin::new(&mut task._handle).poll(cx) {
            Poll::Ready(Ok(res)) => {
                Poll::Ready(Ok(()))
            },
            Poll::Ready(Err(res)) => {
                Poll::Ready(Err(UdpServerHandleError::TokioError(res.to_string())))
            }
            Poll::Pending => todo!(),
        }
    }
}

async fn handle_datagram(len: usize, mut buf: Box<[u8]>) -> ReceiverResult<AudioPacket> {
    // whatever
    /*let mixer = GLOBAL_MASTER_OUTPUT_MIXER.lock().await;
    let mixer = mixer.as_ref().expect("failed to open mixer");*/

    trace!("UDP Packet Length: {:?}", len);
    Ok(bincode2::deserialize(&mut buf[..len])
        .map_err(|e| UdpServerHandleError::SerializationError)?)
    //.map_err(|e| UdpError::DeserializeError(e)).unwrap();
    /*trace!(
        "Received Packet from {}, Length: {}, {:?}",
        addr, len, packet
    );*/
}

async fn udp_server(
    sock: UdpSocket,
    mut ch: mpsc::Receiver<UdpServerCommands>,
    //mixer: SharedInputMixer,
    //raw_ch: &AsyncMixerInputEnd,
    stream_track_selector: MixerTrackSelector,
) -> Result<(), UdpServerHandleError> {
    info!("Starting UDP Receiver");
    // start connection

    loop {
        let mut buf = vec![0; 10000 as usize].into_boxed_slice();
        tokio::select! {
            result = sock.recv_from(&mut buf) => {
                match result {
                    Ok((len, addr)) => {
                        //info!("Received {:?} bytes from {:?}, Payload: {:?}", len, addr, data);

                        match handle_datagram(len, buf).await {
                            Ok(packet) => {

                                trace!("UDP Packet Length: {:?}", len);
                                trace!("Received Packet from {}, Length: {}, {:?}", addr, len, packet);

                                // whatever
                                let mixer = GLOBAL_MASTER_OUTPUT_MIXER.lock().await;
                                let mixer = mixer.as_ref().expect("failed to open mixer");

                                let data: &[f32] = bytemuck::try_cast_slice(&packet.payload).unwrap();

                                //.map_err(|e| UdpError::DeserializeError(e)).unwrap();


                                write_to_mixer_async(mixer, data, stream_track_selector).await;
                            },
                            Err(e) => {

                            }
                        }

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

#[cfg(test)]
pub(crate) mod tests {
    use std::net::SocketAddr;

    use log::debug;
    use tokio::{io, sync::mpsc};

    use crate::{mixer::MixerTrackSelector, streamer::receiver::AudioReceiverHandle};

    pub async fn dummy_receiver(
        _: MixerTrackSelector,
        //smprt: u32,
        //chcount: usize,
    ) -> io::Result<AudioReceiverHandle> {
        let local_addr: SocketAddr = "0.0.0.0:12345".parse().unwrap();
        debug!("dummy server addr: {}", local_addr);
        let (s, r) = mpsc::channel(1);
        Ok(AudioReceiverHandle {
            _handle: tokio::spawn(async move {
                assert!(true);
                Ok(())
            }),
            channel: s,
            local_addr,
        })
    }
}
