use std::{
    fmt::Debug,
    net::{SocketAddr, UdpSocket},
    sync::{mpsc::Sender, Arc, Mutex},
};

use bytemuck::Pod;
use ringbuf::{
    HeapCons, HeapProd,
    traits::{Consumer, Observer, Producer},
};

use crate::streamer_config::StreamerConfig;

use super::streamer::Direction;

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
        config: StreamerConfig,
        target: SocketAddr,
        buffer_consumer: Arc<Mutex<HeapCons<T>>>,
        buffer_producer: Arc<Mutex<HeapProd<T>>>,
        stats: Sender<UdpStats>,
    ) -> std::io::Result<()> {
        match direction {
            Direction::Sender => {
                let socket = UdpSocket::bind(target)?;
                let f = format!("{}:{}", target, config.port);
                println!("Connecting UDP to {}", f);
                socket.connect(f)?;

                Self::udp_sender_loop(&config, socket, buffer_consumer, stats);
            }
            Direction::Receiver => {
                let socket =
                    UdpSocket::bind(("0.0.0.0:42069", config.port))?;

                Self::udp_receiver_loop(&config, socket, buffer_producer, stats);
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
        streamer_config: &StreamerConfig,
        socket: UdpSocket,
        buffer_consumer: Arc<Mutex<HeapCons<T>>>,
        stats: Sender<UdpStats>,
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

                let _ = socket.send(udp_packet);

                // Send statistics to the channel
                if streamer_config.send_network_stats {
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
        streamer_config: &StreamerConfig,
        socket: UdpSocket,
        buffer_producer: Arc<Mutex<HeapProd<T>>>,
        stats: Sender<UdpStats>,
    ) {
        // How big is one byte?
        let byte_size = size_of::<T>();

        //let buffer_producer = self.udp_get_producer();

        //let stats = self.get_udp_stats_sender();

        loop {
            let mut prod = buffer_producer.lock().unwrap();
            let cap: usize = prod.capacity().into();

            // create the temporary network buffer needed to capture the network samples
            let mut temp_network_buffer: Box<[u8]> = vec![0u8; cap * byte_size].into_boxed_slice();

            // Receive from the Network
            match socket.recv(&mut temp_network_buffer) {
                Ok(received) => {
                    // Convert the buffered network samples to the specified sample format
                    let converted_samples: &[T] = bytemuck::cast_slice(&temp_network_buffer);

                    let pre_occupied_buffer = prod.occupied_len();

                    // Transfer Samples bytewise
                    for &sample in converted_samples {
                        // TODO implement fell-behind logic here
                        let _ = prod.try_push(sample);
                    }

                    let post_occupied_buffer = prod.occupied_len();

                    // Send Statistics about the current operation to the stats channel
                    if streamer_config.send_network_stats {
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
