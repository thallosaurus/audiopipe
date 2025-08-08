use std::{net::SocketAddr, pin::Pin, sync::Arc, time::SystemTime};

use log::{error, info, trace};
use ringbuf::traits::{Consumer, Observer, Producer};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{self},
    net::UdpSocket,
    sync::{
        Mutex,
        mpsc::{self, error::SendError},
    },
};

use crate::async_comp::audio::{GLOBAL_MASTER_INPUT, InputMixer, SharedInputMixer};

pub enum UdpServerCommands {
    Stop,
}
pub enum UdpClientCommands {
    Stop,
}

pub struct UdpServerHandle {
    _handle: Pin<Box<dyn Future<Output = io::Result<()>> + Send>>,
    channel: mpsc::Sender<UdpServerCommands>,
    pub local_addr: SocketAddr,
}

impl UdpServerHandle {
    async fn stop(&self) -> Result<(), SendError<UdpServerCommands>> {
        self.channel.send(UdpServerCommands::Stop).await
    }
}

pub struct UdpClientHandle {
    _handle: Pin<Box<dyn Future<Output = io::Result<()>> + Send>>,
    channel: mpsc::Sender<UdpClientCommands>,
}

impl UdpClientHandle {
    async fn stop(&self) -> Result<(), SendError<UdpClientCommands>> {
        self.channel.send(UdpClientCommands::Stop).await
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct TokioUdpAudioPacket {
    sequence: u64,
    timestamp: SystemTime,
    channels: Vec<usize>,
    data: Vec<u8>,
}

pub async fn start_audio_stream_server(
    smprt: u32,
    bufsize: u32,
    chcount: usize,
    mixer: SharedInputMixer,
) -> UdpServerHandle {
    let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    let local_addr = sock.local_addr().unwrap();
    info!("local udp addr: {}", local_addr);

    let (s, r) = mpsc::channel(1);

    let mixer_clone = mixer.clone();
    UdpServerHandle {
        _handle: Box::pin(udp_server(sock, r, mixer)), //info!("UDP Server Listening");
        channel: s,
        local_addr,
    }
}

pub async fn start_audio_stream_client(
    addr: SocketAddr,
    //smprt: u32,
    bufsize: u32,
    chcount: usize,
) -> UdpClientHandle {
    let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();

    info!("Connecting UDP to {}", addr);
    sock.connect(addr).await.unwrap();

    let (s, r) = mpsc::channel(1);

    //info!("Starting UDP Sender");
    UdpClientHandle {
        _handle: Box::pin(udp_client(sock, bufsize * chcount as u32, r)),
        channel: s,
    }
}

pub async fn udp_server(
    sock: UdpSocket,
    mut ch: mpsc::Receiver<UdpServerCommands>,
    mixer: SharedInputMixer,
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
                        process_udp_server_output(packet, &mixer).await;


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
async fn process_udp_server_output(packet: TokioUdpAudioPacket, mixer: &SharedInputMixer) {
    // Convert the buffered network samples to the specified sample format
    let converted_samples: &[f32] = bytemuck::try_cast_slice(&packet.data).unwrap();
    //.map_err(|e| UdpError::CastingError(e))?;
    //let mut output = GLOBAL_MASTER_OUTPUT.lock().await;

    let mut dropped = 0;
    let mut consumed = 0;

    let mut b = 0;

    let mut mixer = mixer.lock().await;
    //mixer.get_channel(channel)

    for &sample in converted_samples {
        // TODO implement fell-behind logic here
        let ch_selector = b % packet.channels.len();
        let channel = mixer.get_channel(ch_selector);

        if let Err(err) = channel.try_push(sample) {
            dropped += 1;
        } else {
            consumed += 1;
        }

        b += 1;
    }
}

const MAX_UDP_CLIENT_PAYLOAD_SIZE: usize = 512;

pub async fn udp_client(
    sock: UdpSocket,
    bufsize: u32,
    mut ch: mpsc::Receiver<UdpClientCommands>,
) -> io::Result<()> {
    info!("Starting UDP Sender");
    let mut input = GLOBAL_MASTER_INPUT.lock().await;
    let mut sequence = 0;
    loop {
        if let Some(cons) = input.as_mut() {
            if !cons.is_empty() {
                let mut buf: Vec<f32> = vec![0.0f32; MAX_UDP_CLIENT_PAYLOAD_SIZE];
                let consumed = cons.pop_slice(&mut buf);

                let data: &[u8] = bytemuck::try_cast_slice(&buf[..consumed]).unwrap();

                let packet = TokioUdpAudioPacket {
                    data: data.to_vec(),
                    sequence,
                    channels: vec![0, 1],
                    timestamp: SystemTime::now(),
                };

                sequence += 1;
                trace!(
                    "Sequence {}, consumed: {}, {:?}",
                    sequence, consumed, packet
                );

                let set = bincode2::serialize(&packet).expect("error while serializing audio data");

                tokio::select! {
                    Ok(sent) = sock.send(&set) => {
                        trace!("Sent {} bytes", sent);
                    },
                    Some(cmd) = ch.recv() => {
                        match cmd {
                            UdpClientCommands::Stop => break,
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::{
        net::UdpSocket,
        sync::{Mutex, mpsc},
    };

    use crate::async_comp::{
        audio::default_mixer,
        udp::{udp_client, udp_server},
    };

    #[tokio::test]
    async fn tcp_server_test() {
        env_logger::init();
    }

    #[tokio::test]
    async fn udp_server_test() {
        env_logger::init();

        let (s, r) = mpsc::channel(1);

        let mixer = default_mixer(2, 1024);

        let input_mixer = Arc::new(Mutex::new(mixer.1));

        let handle = tokio::spawn(async move {
            let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();

            udp_server(sock, r, input_mixer).await.unwrap();
            println!("Server stopped");
        });

        let (s, r) = mpsc::channel(1);

        let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        let c = udp_client(sock, 2, r).await.unwrap();
        /*c.send(String::from("Hello World").as_bytes())
        .await
        .unwrap();*/
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_server_handle_stop() {
        //let server = Arc::new(Mutex::new(start_audio_stream_server(44100, 1024, 2).await));
    }
}
