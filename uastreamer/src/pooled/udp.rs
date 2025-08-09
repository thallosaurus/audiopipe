use std::{
    fmt::Debug,
    io::{self, ErrorKind},
    net::{SocketAddr, UdpSocket},
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender},
    },
    time::{Duration, SystemTime},
};

use bytemuck::{Pod, PodCastError};
use log::{info, trace};
use ringbuf::{
    HeapCons, HeapProd,
    traits::{Consumer, Observer, Producer},
};
use serde::{Deserialize, Serialize};

use crate::Direction;

#[derive(Debug)]
#[deprecated]
pub enum UdpError {
    BindFailed(io::Error),
    ConnectError(io::Error),
    CastingError(PodCastError),
    UdpReceiveError(io::Error),
    SerializeError(Box<bincode2::ErrorKind>),
    DeserializeError(Box<bincode2::ErrorKind>),
    CouldNotDeactivateTimeout(io::Error),
}

#[deprecated]
pub type UdpResult<T> = Result<T, UdpError>;

#[deprecated]
pub enum UdpStatus {
    DidEnd,
}

#[derive(Debug)]
#[deprecated]
pub enum UdpReceiverCommands {
    Stop,
}

#[derive(Debug)]
#[deprecated]
pub enum UdpSenderCommands {
    Stop,
}

const MAX_UDP_PACKET_LENGTH: usize = 65535;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[deprecated]
pub struct UdpAudioPacket {
    sequence: u64,
    timestamp: SystemTime,
    data: Vec<u8>,
}

/// Stats which get sent after each UDP Event
#[derive(Default)]
#[deprecated]
pub struct NetworkUDPStats {
    //pub sent: Option<usize>,
    //pub received: Option<usize>,
    pub consumed: usize,
    pub dropped: usize,
}

#[deprecated]
pub trait UdpStreamFlow<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> {
    fn construct_udp_stream(
        direction: Direction,
        target: SocketAddr,
        buffer_consumer: Arc<Mutex<HeapCons<T>>>,
        buffer_producer: Arc<Mutex<HeapProd<T>>>,
        stats: Option<Sender<NetworkUDPStats>>,
        udp_msg_rx: Receiver<UdpStatus>,
        chan_sync: Receiver<bool>,
    ) -> UdpResult<()> {
        //let (udp_msg_tx, udp_msg_rx) = channel::<UdpStatus>();

        match direction {
            Direction::Sender => {
                let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| UdpError::BindFailed(e))?;
                //let f = format!("{}:{}", target, config.port);
                info!("Connecting UDP to {}", target);
                socket
                    .connect(target)
                    .map_err(|e| UdpError::ConnectError(e))?;

                Self::udp_sender_loop(
                    socket,
                    buffer_consumer,
                    stats,
                    //udp_channel,
                    udp_msg_rx,
                    chan_sync,
                )?;
            }
            Direction::Receiver => {
                // The receiver listens on local_port:ip.
                let socket = UdpSocket::bind(target).map_err(|e| UdpError::BindFailed(e))?;
                info!("Receiving UDP on {}", target);

                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .map_err(|e| UdpError::CouldNotDeactivateTimeout(e))?;

                Self::udp_receiver_loop(
                    socket,
                    buffer_producer,
                    stats,
                    //udp_channel,
                    udp_msg_rx,
                )?;
            }
        }

        Ok(())
    }

    fn udp_get_producer(&self) -> Arc<Mutex<HeapProd<T>>>;
    fn udp_get_consumer(&self) -> Arc<Mutex<HeapCons<T>>>;
    fn get_udp_stats_sender(&self) -> Sender<NetworkUDPStats>;

    /// Entry Point for the UDP Buffer Sender.
    /// Sends the buffer when it is full
    fn udp_sender_loop(
        socket: UdpSocket,
        buffer_consumer: Arc<Mutex<HeapCons<T>>>,
        stats: Option<Sender<NetworkUDPStats>>,
        udp_msg: Receiver<UdpStatus>,
        chan_sync: Receiver<bool>,
    ) -> UdpResult<()> {
        let mut seq = 0;
        loop {
            let mut consumed = 0;
            let dropped = 0;
            if let Ok(msg) = udp_msg.try_recv() {
                match msg {
                    // Graceful Shutdown
                    UdpStatus::DidEnd => {
                        info!("Exiting UDP Sender Loop");
                        break;
                    }
                }
            }

            let mut buffer_consumer = buffer_consumer.lock().unwrap();

            // Only send the network package if the network buffer is full or we got the signal
            if chan_sync.try_recv().unwrap_or(false) || buffer_consumer.is_full() {
                // find out if this might be the epicenter of the glitches
                // 08.05.2025: no, but removing it makes bytemuck on receiver side angry somehow - TODO
                while !buffer_consumer.is_empty() {
                    let mut data_buf: Box<[T]> =
                        vec![T::default(); MAX_UDP_PACKET_LENGTH].into_boxed_slice();
                    consumed += buffer_consumer.pop_slice(&mut data_buf);
                    trace!("Consumed {} bytes", consumed);
                    let udp_data: &[u8] = bytemuck::try_cast_slice(&data_buf[..consumed])
                        .map_err(|e| UdpError::CastingError(e))?;

                    let packet = UdpAudioPacket {
                        //sequence: seq,
                        //total_len: consumed * size_of::<u8>(),
                        //data_len: udp_data.len(),
                        data: udp_data.to_vec(),
                        sequence: seq,
                        timestamp: SystemTime::now(),
                    };
                    trace!("> UDP Packet (Seq: #{}: {:?}", seq, packet);

                    let set =
                        bincode2::serialize(&packet).map_err(|e| UdpError::SerializeError(e))?;
                    trace!("> UDP Packet Serialized: {:?}", set);

                    let _sent_s = socket.send(&set).unwrap();
                    seq += 1;
                }
            }

            // Send Statistics about the current operation to the stats channel
            /*if let Some(ref s) = stats {
                s.send(NetworkUDPStats { consumed, dropped }).unwrap();
            }*/
        }
        Ok(())
    }

    /// Entry Point for the UDP Receiver Loop
    fn udp_receiver_loop(
        //streamer_config: &StreamerConfig,
        socket: UdpSocket,
        buffer_producer: Arc<Mutex<HeapProd<T>>>,
        stats: Option<Sender<NetworkUDPStats>>,
        //udp_channel: Receiver<bool>,
        udp_msg: Receiver<UdpStatus>,
    ) -> UdpResult<()> {
        //let buffer_producer = self.udp_get_producer();

        //let stats = self.get_udp_stats_sender();

        // create the temporary network buffer needed to capture the network samples
        let mut temp_network_buffer = vec![0u8; MAX_UDP_PACKET_LENGTH].into_boxed_slice();

        loop {
            //println!("Inside Receiver Loop, {}", socket.local_addr().unwrap());
            let mut prod = buffer_producer.lock().unwrap();
            let mut consumed = 0;
            let mut dropped = 0;
            //let cap: usize = prod.capacity().into();

            if let Ok(msg) = udp_msg.try_recv() {
                match msg {
                    UdpStatus::DidEnd => {
                        info!("Exiting UDP Receiver Loop");
                        break;
                    }
                }
            }

            // Receive from the Network
            match socket.recv(&mut temp_network_buffer) {
                Ok(received) => {
                    let packet: UdpAudioPacket =
                        bincode2::deserialize_from(&temp_network_buffer[..received])
                            .map_err(|e| UdpError::DeserializeError(e))?;
                    trace!("Received Packet: {:?}", packet);

                    // Convert the buffered network samples to the specified sample format
                    let converted_samples: &[T] = bytemuck::try_cast_slice(&packet.data)
                        .map_err(|e| UdpError::CastingError(e))?;

                    trace!("Converted Packet: {:?}", converted_samples);
                    // Transfer Samples bytewise
                    for &sample in converted_samples {
                        // TODO implement fell-behind logic here
                        if let Err(err) = prod.try_push(sample) {
                            dropped += 1;
                        } else {
                            consumed += 1;
                        }
                    }
                    //let pre_occupied_buffer = prod.occupied_len();

                    //let post_occupied_buffer = prod.occupied_len();
                }
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut {
                        // signals that there is no data available. just continue with the loop
                        // until data becomes available
                        continue;
                    } else {
                        //error!("UDP receive error: {:?}", e);
                        return Err(UdpError::UdpReceiveError(e));
                    }
                }
            }

            // Send Statistics about the current operation to the stats channel
            /*if let Some(ref s) = stats {
                s.send(NetworkUDPStats { consumed, dropped }).unwrap();
            }*/
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    const TEST_DATA: &'static [u8] = "hxte-thx-wxrld".as_bytes();
    //const _debug_udp_address: &'static str = "127.0.0.1:12345";

    use std::{
        net::SocketAddr,
        str::FromStr,
        sync::{
            Arc, Mutex,
            mpsc::{Sender, channel},
        },
        thread,
        time::Duration,
    };

    use log::info;
    use rand::Rng;
    use ringbuf::{
        HeapCons, HeapProd,
        traits::{Observer, Producer, Split},
    };

    use crate::pooled::{cpal::CpalStats, AppDebug};

    use super::{NetworkUDPStats, UdpStatus, UdpStreamFlow};

    struct UdpTransportDebugAdapter {
        audio_buffer_prod: Arc<Mutex<HeapProd<u8>>>,
        audio_buffer_cons: Arc<Mutex<HeapCons<u8>>>,
        _cpal_stats_sender: Sender<CpalStats>,
        udp_stats_sender: Sender<NetworkUDPStats>,
        //config: StreamerConfig,
        //pub pool: ThreadPool
    }

    impl UdpTransportDebugAdapter {
        pub fn new(buffer_size: usize) -> (Self, AppDebug) {
            let audio_buffer = ringbuf::HeapRb::<u8>::new(buffer_size);

            info!("Buffer Capacity: {}", audio_buffer.capacity());

            let (audio_buffer_prod, audio_buffer_cons) = audio_buffer.split();

            let (_cpal_stats_sender, cpal_stats_receiver) = channel::<CpalStats>();
            let (udp_stats_sender, udp_stats_receiver) = channel::<NetworkUDPStats>();

            (
                UdpTransportDebugAdapter {
                    audio_buffer_prod: Arc::new(Mutex::new(audio_buffer_prod)),
                    audio_buffer_cons: Arc::new(Mutex::new(audio_buffer_cons)),
                    _cpal_stats_sender,
                    udp_stats_sender,
                },
                AppDebug {
                    cpal_stats_receiver,
                    udp_stats_receiver,
                },
            )
        }
    }

    impl UdpStreamFlow<u8> for UdpTransportDebugAdapter {
        fn udp_get_producer(&self) -> std::sync::Arc<std::sync::Mutex<ringbuf::HeapProd<u8>>> {
            self.audio_buffer_prod.clone()
        }

        fn udp_get_consumer(&self) -> std::sync::Arc<std::sync::Mutex<ringbuf::HeapCons<u8>>> {
            self.audio_buffer_cons.clone()
        }

        fn get_udp_stats_sender(&self) -> std::sync::mpsc::Sender<super::NetworkUDPStats> {
            self.udp_stats_sender.clone()
        }
    }

    fn random_port() -> u16 {
        let mut rng = rand::rng();
        rng.random_range(9000..9999)
    }

    #[test]
    fn test_cancel_channel() {
        let receiver_addr = SocketAddr::from_str("127.0.0.1:0").unwrap();
        let (tx1, rx) = channel::<bool>();
        let (udp_msg_tx, udp_msg_rx) = channel::<UdpStatus>();

        let (sender, _) = UdpTransportDebugAdapter::new(TEST_DATA.len());

        let t = thread::spawn(move || {
            UdpTransportDebugAdapter::construct_udp_stream(
                crate::Direction::Receiver,
                receiver_addr,
                sender.audio_buffer_cons,
                sender.audio_buffer_prod,
                Some(sender.udp_stats_sender),
                udp_msg_rx,
                rx,
            )
            .unwrap();
        });

        udp_msg_tx.send(UdpStatus::DidEnd).unwrap();

        t.join().unwrap();
    }

    #[test]
    fn test_packet_fragmentation() {}

    #[test]
    fn flow_sends_when_buffer_is_full() {
        //let port = random_port();
        let mut sender_addr = SocketAddr::from_str("0.0.0.0:0").unwrap();

        let port = random_port();
        let receiver_addr = SocketAddr::from_str(&format!("0.0.0.0:{}", port)).unwrap();
        sender_addr.set_port(port);

        let (sender, _) = UdpTransportDebugAdapter::new(TEST_DATA.len());
        let (receiver, _) = UdpTransportDebugAdapter::new(TEST_DATA.len());

        let input_prod = sender.audio_buffer_prod.clone();
        let output_cons = receiver.audio_buffer_cons.clone();

        let cons = receiver.audio_buffer_cons.clone();
        let prod = receiver.audio_buffer_prod.clone();

        let (tx1, rx) = channel::<bool>();
        let (udp_msg_tx, udp_msg_rx) = channel::<UdpStatus>();

        // Receiver
        thread::spawn(move || {
            UdpTransportDebugAdapter::construct_udp_stream(
                crate::Direction::Receiver,
                receiver_addr,
                cons,
                prod,
                Some(receiver.udp_stats_sender),
                udp_msg_rx,
                rx,
            )
            .unwrap();
        });

        let cons = sender.audio_buffer_cons.clone();
        let prod = sender.audio_buffer_prod.clone();

        let (tx2, rx) = channel::<bool>();
        let (udp_msg_tx, udp_msg_rx) = channel::<UdpStatus>();

        thread::spawn(move || {
            UdpTransportDebugAdapter::construct_udp_stream(
                crate::Direction::Sender,
                sender_addr,
                cons,
                prod,
                Some(sender.udp_stats_sender),
                udp_msg_rx,
                rx,
            )
            .unwrap();
        });

        let mut input_prod = input_prod.lock().unwrap();
        for d in TEST_DATA.iter() {
            input_prod.try_push(*d).unwrap();
        }

        std::thread::sleep(Duration::from_secs(1));

        let output_cons = output_cons.lock().unwrap();
        //cons.pop_slice(&mut v);
        tx1.send(true).unwrap();
        tx2.send(true).unwrap();

        // Input Producer must be empty, because its contents were sent
        assert_eq!(input_prod.occupied_len(), 0);

        assert_eq!(output_cons.occupied_len(), TEST_DATA.len());
    }

    #[test]
    fn flow_wont_send_when_buffer_is_not_full() {
        let stub_tcp_sender_addr =
            SocketAddr::from_str(&format!("127.0.0.1:{}", random_port())).unwrap();
        let stub_tcp_receiver_addr =
            SocketAddr::from_str(&format!("127.0.0.1:{}", random_port())).unwrap();

        let (sender, _) = UdpTransportDebugAdapter::new(1024);
        let (receiver, _) = UdpTransportDebugAdapter::new(1024);

        let input_prod = sender.audio_buffer_prod.clone();
        let output_cons = receiver.audio_buffer_cons.clone();

        let (udp1_tx, rx1) = channel::<bool>();
        let (udp2_tx, rx2) = channel::<bool>();
        {
            let cons = receiver.audio_buffer_cons.clone();
            let prod = receiver.audio_buffer_prod.clone();
            let (udp_msg_tx, udp_msg_rx) = channel::<UdpStatus>();

            // Receiver
            thread::spawn(move || {
                UdpTransportDebugAdapter::construct_udp_stream(
                    crate::Direction::Receiver,
                    stub_tcp_receiver_addr,
                    cons,
                    prod,
                    Some(receiver.udp_stats_sender),
                    udp_msg_rx,
                    rx1,
                )
                .unwrap();
            });

            let cons = sender.audio_buffer_cons.clone();
            let prod = sender.audio_buffer_prod.clone();
            let (udp_msg_tx, udp_msg_rx) = channel::<UdpStatus>();

            thread::spawn(move || {
                UdpTransportDebugAdapter::construct_udp_stream(
                    crate::Direction::Sender,
                    stub_tcp_sender_addr,
                    cons,
                    prod,
                    Some(sender.udp_stats_sender),
                    udp_msg_rx,
                    rx2,
                )
                .unwrap();
            });
        }

        let mut input_prod = input_prod.lock().unwrap();
        for d in TEST_DATA.iter() {
            input_prod.try_push(*d).unwrap();
        }

        std::thread::sleep(Duration::from_secs(1));

        let output_cons = output_cons.lock().unwrap();
        //cons.pop_slice(&mut v);
        assert_eq!(input_prod.occupied_len(), TEST_DATA.len());
        assert_eq!(output_cons.occupied_len(), 0);

        udp1_tx.send(true).unwrap();
        udp2_tx.send(true).unwrap();
    }
}
