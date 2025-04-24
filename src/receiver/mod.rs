use std::{net::UdpSocket, sync::{mpsc::{self, Receiver}, Arc, Mutex}};

use cpal::{traits::{DeviceTrait, StreamTrait}, Device, Stream, StreamConfig};
use ringbuf::traits::{Consumer, Observer, Producer, Split};

use crate::create_wav_writer;

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
        buf_size: u32,
        dump_received: bool,
        port: u16
    ) -> anyhow::Result<AudioReceiver> {
        //let device = host.default_output_device().expect("no output device available");
        //let config = device.default_output_config()?;
        //println!("Sample format: {:?}", config.sample_format());

        let (tx, udp_rx) = mpsc::channel::<UdpStats>();

        let ring = ringbuf::HeapRb::<f32>::new(buf_size as usize);
        let (producer, mut consumer) = ring.split();
        let producer = Arc::new(Mutex::new(producer));

        let producer_clone = Arc::clone(&producer);
        std::thread::spawn(move || {
            let socket = UdpSocket::bind(("0.0.0.0", port)).expect("Failed to bind UDP socket");
            println!("Listening on UDP port {}", port);

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

        let mut debug_sample_writer = create_wav_writer("receiver_dump.wav".to_owned(), 1, 44100, 32, hound::SampleFormat::Float)?;
        let stream = device.build_output_stream(
            &config.into(),
            move |output: &mut [f32], _| {

                let consumed = consumer.pop_slice(output);

                if dump_received && consumed > 0 {  // Only dump when there also was data

                    #[cfg(debug_assertions)]
                    for sample in output.iter() {
                        //*sample = consumer.try_pop().unwrap_or(Sample::EQUILIBRIUM);
                        debug_sample_writer.write_sample(*sample).unwrap();
                    }
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