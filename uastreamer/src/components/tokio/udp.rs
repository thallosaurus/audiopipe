use std::{net::SocketAddr, time::SystemTime};

use cpal::{Device, Stream, traits::DeviceTrait};
use log::{debug, error, info, trace};
use rand::seq;
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{self, AsyncReadExt},
    net::{TcpListener, UdpSocket},
    select,
    sync::mpsc,
    task::JoinHandle,
};

use crate::{
    components::{
        control::TcpControlPacket,
        cpal::select_output_device_config,
        tokio::audio::{GLOBAL_MASTER_INPUT, GLOBAL_MASTER_OUTPUT},
        udp::{UdpAudioPacket, UdpError},
    },
    ualog::SimpleLogger,
};

type UdpServerCommands = bool;
type UdpClientCommands = bool;

pub struct UdpServerHandle {
    _handle: JoinHandle<()>,
    channel: mpsc::Sender<UdpServerCommands>,
    pub local_addr: SocketAddr,
}

pub struct UdpClientHandle {
    _handle: JoinHandle<()>,
    channel: mpsc::Sender<UdpClientCommands>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct TokioUdpAudioPacket {
    sequence: u64,
    timestamp: SystemTime,
    data: Vec<u8>,
}

pub async fn start_audio_stream_server(
    smprt: u32,
    bufsize: u32,
    chcount: usize,
) -> UdpServerHandle {
    let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    let local_addr = sock.local_addr().unwrap();
    info!("local udp addr: {}", local_addr);

    let (s, r) = mpsc::channel(1);
    UdpServerHandle {
        _handle: tokio::spawn(async move {
            //info!("UDP Server Listening");
            udp_server(sock, bufsize * chcount as u32, r).await.unwrap();
        }),
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

    UdpClientHandle {
        _handle: tokio::spawn(async move {
            info!("Starting UDP Sender");
            udp_client(sock, bufsize * chcount as u32, r).await.unwrap();
        }),
        channel: s,
    }
}

pub async fn udp_server(
    sock: UdpSocket,
    bufsize: u32,
    mut ch: mpsc::Receiver<bool>,
) -> io::Result<()> {
    // start connection
    let mut buf = vec![0; 10000 as usize].into_boxed_slice();
    //let mut temp_network_buffer = vec![0u8; MAX_UDP_PACKET_LENGTH].into_boxed_slice();

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
                        process_udp_server_output(packet).await;


                        // decode packet
                        //info!("{:?}", String::from_utf8(data.to_vec()));

                        #[cfg(test)]
                        break
                    },
                    Err(e) => {
                        error!("recv error: {:?}", e);
                    }
                }
            },

            c = ch.recv() => {
                break
            },
        };
    }

    Ok(())
}

async fn process_udp_server_output(packet: TokioUdpAudioPacket) {
    // Convert the buffered network samples to the specified sample format
    let converted_samples: &[f32] = bytemuck::try_cast_slice(&packet.data).unwrap();
    //.map_err(|e| UdpError::CastingError(e))?;
    let mut output = GLOBAL_MASTER_OUTPUT.lock().await;

    let mut dropped = 0;
    let mut consumed = 0;

    if let Some(prod) = output.as_mut() {
        for &sample in converted_samples {
            // TODO implement fell-behind logic here
            if let Err(err) = prod.try_push(sample) {
                dropped += 1;
            } else {
                consumed += 1;
            }
        }
    } else {
        //
    }
}

const MAX_UDP_CLIENT_PAYLOAD_SIZE: usize = 512;

pub async fn udp_client(
    sock: UdpSocket,
    bufsize: u32,
    mut ch: mpsc::Receiver<bool>,
) -> io::Result<()> {
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
                    timestamp: SystemTime::now(),
                };

                sequence += 1;
                trace!("Sequence {}, consumed: {}, {:?}", sequence, consumed, packet);

                //trace!("{:?}", packet);

                let set = bincode2::serialize(&packet).expect("error while serializing audio data");

                tokio::select! {
                    Ok(sent) = sock.send(&set) => {
                        trace!("Sent {} bytes", sent);
                    },
                    Some(flag) = ch.recv() => {
                        info!("Quitting UDP Client");
                        break
                    }
                }
            }
            //sock.send(cons).await;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os;

    use crossbeam::channel;
    use ringbuf::{HeapProd, HeapRb, rb, traits::Split};
    use tokio::{net::UdpSocket, sync::mpsc};

    use crate::components::tokio::udp::{udp_client, udp_server};

    #[tokio::test]
    async fn tcp_server_test() {
        env_logger::init();
    }

    #[tokio::test]
    async fn udp_server_test() {
        env_logger::init();

        let (s, r) = mpsc::channel(1);

        let handle = tokio::spawn(async move {
            let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();

            udp_server(sock, 2, r).await.unwrap();
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
}
