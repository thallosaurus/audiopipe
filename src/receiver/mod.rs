use std::{
    net::UdpSocket,
    sync::mpsc::{self, Receiver, Sender},
    thread::JoinHandle,
};

use bytemuck::Pod;
use cpal::{
    traits::{DeviceTrait, StreamTrait}, Device, Sample, Stream, StreamConfig
};
use ringbuf::traits::{Consumer, Observer, Producer, Split};

use crate::create_wav_writer;

pub mod args;
pub mod tui;

pub struct UdpStats {
    pub received: usize,
    pub occupied_buffer: usize,
}

pub struct CpalStats {
    requested_sample_length: usize,
    consumed: usize,
}

struct AudioReceiver {
    _stream: Stream,
    _socket_loop: JoinHandle<()>,
    pub udp_rx: Receiver<UdpStats>,
    pub cpal_rx: Receiver<CpalStats>,
}

impl AudioReceiver {
    pub fn new(
        device: &Device,
        config: StreamConfig,
        buf_size: u32,
        dump_received: bool,
        port: u16,
    ) -> anyhow::Result<AudioReceiver> {
        let (udp_tx, udp_rx) = mpsc::channel::<UdpStats>();

        let socket = UdpSocket::bind(("0.0.0.0", port)).expect("Failed to bind UDP socket");
        println!("Listening on UDP port {}", port);

        let (cpal_tx, cpal_rx) = mpsc::channel::<CpalStats>();

        let (stream, socket_loop) = Self::build_stream::<f32>(
            &device,
            &config.into(),
            socket,
            cpal_tx,
            udp_tx,
            buf_size,
            dump_received,
        )?;

        stream.play()?;

        Ok(AudioReceiver {
            _stream: stream,
            _socket_loop: socket_loop,
            udp_rx,
            cpal_rx,
        })
    }

    pub fn build_stream<T>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        socket: UdpSocket,
        cpal_tx: Sender<CpalStats>,
        udp_tx: Sender<UdpStats>,
        buf_size: u32,
        dump_received: bool,
    ) -> Result<(cpal::Stream, JoinHandle<()>), anyhow::Error>
    where
        T: cpal::SizedSample + Send + Pod + Default + hound::Sample + 'static,
    {
        let ring = ringbuf::HeapRb::<T>::new((buf_size as usize) * 2);
        let (mut producer, mut consumer) = ring.split();

        #[cfg(debug_assertions)]
        let mut debug_sample_writer = create_wav_writer(
            "receiver_dump.wav".to_owned(),
            1,
            44100,
            32,
            hound::SampleFormat::Float,
        )?;

        let stream = device.build_output_stream(
            config.into(),
            move |output: &mut [T], _| {
                // copies the requested buffer to the output slice, respecting the size of the output slice
                //let mut consumed = 0;
                /*if consumer.is_full() {
                    consumed = consumer.pop_slice(output);
                }*/

                let mut consumed = 0;

                for sample in output.iter_mut() {
                    *sample = consumer.try_pop().unwrap_or(Sample::EQUILIBRIUM);
                    #[cfg(debug_assertions)]
                    if dump_received {
                        debug_sample_writer.write_sample(*sample).unwrap();
                    }

                    consumed += 1;
                }

                cpal_tx
                    .send(CpalStats {
                        requested_sample_length: output.len(),
                        consumed,
                    })
                    .unwrap();
            },
            |err| eprintln!("Stream error: {}", err),
            None,
        )?;

        let udp_loop = std::thread::spawn(move || {
            let t_size = size_of::<T>();

            let mut raw_buf: Box<[u8]> = vec![0u8; (buf_size as usize) * t_size].into_boxed_slice();

            loop {
                match socket.recv(&mut raw_buf) {
                    Ok(received) => {
                        let float_samples: &[T] = bytemuck::cast_slice(&raw_buf);

                        //let mut prod = producer_clone.lock().unwrap();

                        for &sample in float_samples {
                            // TODO implement fell-behind logic here
                            let _ = producer.try_push(sample);
                        }

                        let occupied_buffer = producer.occupied_len();

                        udp_tx
                            .send(UdpStats {
                                received,
                                occupied_buffer,
                            })
                            .unwrap();
                    }
                    Err(e) => eprintln!("UDP receive error: {:?}", e),
                }
            }
        });

        Ok((stream, udp_loop))
    }
}
