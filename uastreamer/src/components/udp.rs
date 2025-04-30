use std::{
    fmt::Debug,
    net::{IpAddr, SocketAddr, UdpSocket},
    str::FromStr,
    sync::{Arc, Mutex, mpsc::Sender},
    time::Duration,
};

use bytemuck::Pod;
use ringbuf::{
    HeapCons, HeapProd,
    traits::{Consumer, Observer, Producer},
};

use crate::Direction;

/// Stats which get sent after each UDP Event
#[derive(Default)]
pub struct UdpStats {
    pub sent: Option<usize>,
    pub received: Option<usize>,
    pub pre_occupied_buffer: usize,
    pub post_occupied_buffer: usize,
}

pub trait UdpStreamFlow<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> {
    fn construct_udp_stream(
        direction: Direction,
        target: SocketAddr,
        buffer_consumer: Arc<Mutex<HeapCons<T>>>,
        buffer_producer: Arc<Mutex<HeapProd<T>>>,
        stats: Sender<UdpStats>,
        send_network_stats: bool,
    ) -> std::io::Result<()> {
        match direction {
            Direction::Sender => {
                let socket = UdpSocket::bind("0.0.0.0:0")?;
                //let f = format!("{}:{}", target, config.port);
                println!("[FLOW] Connecting UDP to {}", target);
                socket.connect(target)?;

                Self::udp_sender_loop(socket, buffer_consumer, stats, send_network_stats);
            }
            Direction::Receiver => {
                let socket = UdpSocket::bind(target)?;

                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();

                Self::udp_receiver_loop(socket, buffer_producer, stats, send_network_stats);
            }
        }

        Ok(())
    }

    fn udp_get_producer(&self) -> Arc<Mutex<HeapProd<T>>>;
    fn udp_get_consumer(&self) -> Arc<Mutex<HeapCons<T>>>;
    fn get_udp_stats_sender(&self) -> Sender<UdpStats>;

    /// Entry Point for the UDP Buffer Sender.
    /// Sends the buffer when it is full
    fn udp_sender_loop(
        //streamer_config: &StreamerConfig,
        socket: UdpSocket,
        buffer_consumer: Arc<Mutex<HeapCons<T>>>,
        stats: Sender<UdpStats>,
        send_network_stats: bool,
    ) {
        loop {
            let mut buffer_consumer = buffer_consumer.lock().unwrap();

            // Only send the network package if the network buffer is full to avoid partial sends
            if buffer_consumer.is_full() {
                // TODO Check if this might slow down communication
                let mut network_buffer: Box<[T]> =
                    vec![T::default(); buffer_consumer.capacity().into()].into_boxed_slice();

                // get buffer size before changes
                let pre_occupied_buffer = buffer_consumer.occupied_len();

                // Place the network buffer onto the stack
                buffer_consumer.pop_slice(&mut network_buffer);

                // Occupied Size after operation
                let post_occupied_buffer = buffer_consumer.occupied_len();

                // The Casted UDP Packet
                let udp_packet: &[u8] = bytemuck::cast_slice(&network_buffer);

                let sent_s = socket.send(udp_packet).unwrap();

                // Send statistics to the channel
                if send_network_stats {
                    stats
                        .send(UdpStats {
                            sent: Some(udp_packet.len()),
                            received: None,
                            pre_occupied_buffer,
                            post_occupied_buffer,
                        })
                        .unwrap();
                }
            }
        }
    }

    /// Entry Point for the UDP Receiver Loop
    fn udp_receiver_loop(
        //streamer_config: &StreamerConfig,
        socket: UdpSocket,
        buffer_producer: Arc<Mutex<HeapProd<T>>>,
        stats: Sender<UdpStats>,
        send_network_stats: bool,
    ) {
        // How big is one byte?
        let byte_size = size_of::<T>();

        //let buffer_producer = self.udp_get_producer();

        //let stats = self.get_udp_stats_sender();

        
        loop {
            //println!("Inside Receiver Loop, {}", socket.local_addr().unwrap());
            let mut prod = buffer_producer.lock().unwrap();
            let cap: usize = prod.capacity().into();

            dbg!(cap);

            // create the temporary network buffer needed to capture the network samples
            let mut temp_network_buffer: Box<[u8]> = vec![0u8; cap * byte_size].into_boxed_slice();

            // Receive from the Network
            match socket.recv(&mut temp_network_buffer) {
                Ok(received) => {
                    // Convert the buffered network samples to the specified sample format
                    //println!("UDP Received a packet... {}", received);
                    let converted_samples: &[T] = bytemuck::cast_slice(&temp_network_buffer);

                    let pre_occupied_buffer = prod.occupied_len();

                    // Transfer Samples bytewise
                    for &sample in converted_samples {
                        // TODO implement fell-behind logic here
                        let _ = prod.try_push(sample);
                    }

                    let post_occupied_buffer = prod.occupied_len();

                    // Send Statistics about the current operation to the stats channel
                    if send_network_stats {
                        stats
                            .send(UdpStats {
                                sent: None,
                                received: Some(received),
                                pre_occupied_buffer,
                                post_occupied_buffer,
                            })
                            .unwrap();
                    }
                }
                Err(e) => eprintln!("UDP receive error: {:?}", e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    const TEST_DATA: &'static [u8] = "hxte-thx-wxrld".as_bytes();
    const debug_udp_address: &'static str = "127.0.0.1:12345";

    use std::{
        net::SocketAddr,
        str::FromStr,
        sync::{
            mpsc::{channel, Sender}, Arc, Mutex
        },
        thread, time::Duration,
    };

    use ringbuf::{
        traits::{Observer, Producer, Split}, HeapCons, HeapProd
    };

    use crate::{AppDebug, components::cpal::CpalStats};

    use super::{UdpStats, UdpStreamFlow};

    struct UdpTransportDebugAdapter {
        audio_buffer_prod: Arc<Mutex<HeapProd<u8>>>,
        audio_buffer_cons: Arc<Mutex<HeapCons<u8>>>,
        cpal_stats_sender: Sender<CpalStats>,
        udp_stats_sender: Sender<UdpStats>,
        //config: StreamerConfig,
        //pub pool: ThreadPool
    }

    impl UdpTransportDebugAdapter {
        pub fn new(buffer_size: usize) -> (Self, AppDebug) {
            let audio_buffer = ringbuf::HeapRb::<u8>::new(buffer_size);

            println!("{}", audio_buffer.capacity());

            let (audio_buffer_prod, audio_buffer_cons) = audio_buffer.split();

            let (cpal_stats_sender, cpal_stats_receiver) = channel::<CpalStats>();
            let (udp_stats_sender, udp_stats_receiver) = channel::<UdpStats>();

            (
                UdpTransportDebugAdapter {
                    audio_buffer_prod: Arc::new(Mutex::new(audio_buffer_prod)),
                    audio_buffer_cons: Arc::new(Mutex::new(audio_buffer_cons)),
                    cpal_stats_sender,
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

        fn get_udp_stats_sender(&self) -> std::sync::mpsc::Sender<super::UdpStats> {
            self.udp_stats_sender.clone()
        }
    }

    #[test]
    fn flow_sends_when_buffer_is_full() {
        let (sender, _) = UdpTransportDebugAdapter::new(TEST_DATA.len());
        let (receiver, _) = UdpTransportDebugAdapter::new(TEST_DATA.len());

        let input_prod = sender.audio_buffer_prod.clone();
        let output_cons = receiver.audio_buffer_cons.clone();

        {
            let cons = receiver.audio_buffer_cons.clone();
            let prod = receiver.audio_buffer_prod.clone();

            // Receiver
            thread::spawn(move || {
                UdpTransportDebugAdapter::construct_udp_stream(
                    crate::Direction::Receiver,
                    SocketAddr::from_str(debug_udp_address).unwrap(),
                    cons,
                    prod,
                    receiver.udp_stats_sender,
                    false,
                )
                .unwrap();
                loop {}
            });

            let cons = sender.audio_buffer_cons.clone();
            let prod = sender.audio_buffer_prod.clone();
            thread::spawn(move || {
                UdpTransportDebugAdapter::construct_udp_stream(
                    crate::Direction::Sender,
                    SocketAddr::from_str(debug_udp_address).unwrap(),
                    cons,
                    prod,
                    sender.udp_stats_sender,
                    false,
                )
                .unwrap();
                loop {}
            });
        }

        
        let mut input_prod = input_prod.lock().unwrap();
        for d in TEST_DATA.iter() {
            input_prod.try_push(*d).unwrap();
        }
        
        std::thread::sleep(Duration::from_secs(1));

        let output_cons = output_cons.lock().unwrap();
        //cons.pop_slice(&mut v);
        assert_eq!(input_prod.occupied_len(), 0);
        assert_eq!(output_cons.occupied_len(), TEST_DATA.len());
    }

    #[test]
    fn flow_wont_send_when_buffer_is_not_full() {
        let (sender, _) = UdpTransportDebugAdapter::new(1024);
        let (receiver, _) = UdpTransportDebugAdapter::new(1024);

        let input_prod = sender.audio_buffer_prod.clone();
        let output_cons = receiver.audio_buffer_cons.clone();

        {
            let cons = receiver.audio_buffer_cons.clone();
            let prod = receiver.audio_buffer_prod.clone();

            // Receiver
            thread::spawn(move || {
                UdpTransportDebugAdapter::construct_udp_stream(
                    crate::Direction::Receiver,
                    SocketAddr::from_str(debug_udp_address).unwrap(),
                    cons,
                    prod,
                    receiver.udp_stats_sender,
                    false,
                )
                .unwrap();
                loop {}
            });

            let cons = sender.audio_buffer_cons.clone();
            let prod = sender.audio_buffer_prod.clone();
            thread::spawn(move || {
                UdpTransportDebugAdapter::construct_udp_stream(
                    crate::Direction::Sender,
                    SocketAddr::from_str(debug_udp_address).unwrap(),
                    cons,
                    prod,
                    sender.udp_stats_sender,
                    false,
                )
                .unwrap();
                loop {}
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
    }
}
