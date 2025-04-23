use std::{net::UdpSocket, sync::{mpsc::{self, Receiver}, Arc, Mutex}};

use cpal::{traits::{DeviceTrait, StreamTrait}, Device, Sample, Stream, StreamConfig};
use ringbuf::traits::{Consumer, Observer, Producer, Split};

use crate::{DEFAULT_PORT, RECEIVER_BUFFER_SIZE};

pub mod tui;
pub mod args;

pub struct UdpStats {
    pub received: usize,
    pub occupied_buffer: usize,
}

pub struct CpalStats {
    requested_sample_length: usize,
}

pub struct AudioReceiver {
    _stream: Stream,
    pub udp_rx: Receiver<UdpStats>,
    pub cpal_rx: Receiver<CpalStats>,
}

impl AudioReceiver {
    pub fn new(
        device: &Device,
        config: StreamConfig,
        closing_rx: Receiver<bool>,
        buf_size: u32
    ) -> anyhow::Result<AudioReceiver> {
        //let device = host.default_output_device().expect("no output device available");
        //let config = device.default_output_config()?;
        println!("Using device: {}", device.name()?);
        //println!("Sample format: {:?}", config.sample_format());

        let (tx, udp_rx) = mpsc::channel::<UdpStats>();

        let ring = ringbuf::HeapRb::<f32>::new(buf_size as usize);
        let (producer, mut consumer) = ring.split();
        let producer = Arc::new(Mutex::new(producer));

        let producer_clone = Arc::clone(&producer);
        std::thread::spawn(move || {
            let socket = UdpSocket::bind(("0.0.0.0", DEFAULT_PORT)).expect("Failed to bind UDP socket");
            println!("Listening on UDP port {}", DEFAULT_PORT);

            //let mut buf = Vec::<u8>::new()
            let mut buf: Box<[u8]> = vec![0u8; buf_size as usize].into_boxed_slice();

            loop {
                match socket.recv(&mut buf) {
                    Ok(received) => {
                        let float_samples: &[f32] = bytemuck::cast_slice(&buf);

                        let mut prod = producer_clone.lock().unwrap();

                        for &sample in float_samples {
                            // TODO implement fell-behind logic here
                            let _ = prod.try_push(sample);
                        }

                        let occupied_buffer = prod.occupied_len();

                        tx.send(UdpStats {
                            received,
                            occupied_buffer,
                        }).unwrap();
                    }
                    Err(e) => eprintln!("UDP receive error: {:?}", e),
                }
            }
        });

        let (cpal_tx, cpal_rx) = mpsc::channel::<CpalStats>();

        let stream = device.build_output_stream(
            &config.into(),
            move |output: &mut [f32], _| {
                for sample in output.iter_mut() {
                    *sample = consumer.try_pop().unwrap_or(Sample::EQUILIBRIUM);
                }

                cpal_tx.send(CpalStats {
                    requested_sample_length: output.len(),
                }).unwrap();
            },
            |err| eprintln!("Stream error: {}", err),
            None,
        )?;

        stream.play()?;

        Ok(AudioReceiver {
            _stream: stream,
            udp_rx,
            cpal_rx,
        })
    }
}