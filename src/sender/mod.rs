use core::net;
use std::{net::UdpSocket, sync::mpsc::{channel, Receiver, Sender}, thread::JoinHandle};

use bytemuck::Pod;
use cpal::{
    Device, Stream, StreamConfig,
    traits::{DeviceTrait, StreamTrait},
};
use ringbuf::{
    HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};

pub mod tui;
pub mod args;

pub struct UdpStats {
    pub sent: usize,
    pub occupied_buffer: usize,
}

pub struct CpalStats {
    requested_sample_length: usize,
}

pub struct AudioSender {
    _stream: Stream,
    _socket_loop: JoinHandle<()>,
    pub udp_rx: Receiver<UdpStats>,
    pub cpal_rx: Receiver<CpalStats>,
}

impl AudioSender {
    pub fn new(
        device: &Device,
        config: StreamConfig,
        ip: net::Ipv4Addr,
        port: u16,
        buf_size: u32
    ) -> anyhow::Result<AudioSender> {
        println!("Using device: {}", device.name()?);
        //println!("Sample format: {:?}", config.sample_format());

        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.connect(format!("{}:{}", ip, port))?;

        //let err_fn = |err| eprintln!("Stream error: {}", err);
        let (cpal_tx, cpal_rx) = channel();
        let (udp_tx, udp_rx) = channel();

        // currently locked to f32 - TODO
        let (stream, socket_loop) = Self::build_stream::<f32>(&device, &config.into(), socket, cpal_tx, udp_tx, buf_size)?;
        //let (stream, socket_loop) = match config.sample_format() {
        /*  cpal::SampleFormat::I16 => Self::build_stream::<i16>(&device, &config.into(), socket),
            cpal::SampleFormat::U16 => Self::build_stream::<u16>(&device, &config.into(), socket),
            cpal::SampleFormat::I8 => Self::build_stream::<i8>(&device, &config.into(), socket),
            cpal::SampleFormat::I32 => Self::build_stream::<i32>(&device, &config.into(), socket),
            cpal::SampleFormat::I64 => Self::build_stream::<i64>(&device, &config.into(), socket),
            cpal::SampleFormat::U8 => Self::build_stream::<u8>(&device, &config.into(), socket),
            cpal::SampleFormat::U32 => Self::build_stream::<u32>(&device, &config.into(), socket),
            cpal::SampleFormat::U64 => Self::build_stream::<u64>(&device, &config.into(), socket),
            cpal::SampleFormat::F64 => Self::build_stream::<f64>(&device, &config.into(), socket),
            _ => todo!(),
        }?;*/

        stream.play()?;

        //wait_for_key("Stream started... Press enter to stop");
        Ok(AudioSender {
            _stream: stream,
            _socket_loop: socket_loop,
            udp_rx,
            cpal_rx,
            
        })
    }

    fn build_stream<T>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        socket: UdpSocket,
        cpal_tx: Sender<CpalStats>,
        udp_tx: Sender<UdpStats>,
        buf_size: u32,
    ) -> Result<(cpal::Stream, JoinHandle<()>), anyhow::Error>
    where
        T: cpal::SizedSample + Send + Pod + Default + 'static,
    {
        //let channels = config.channels as usize;
        let buf = HeapRb::<T>::new(buf_size as usize);
        let (mut prod, mut cons) = buf.split();

        let stream = device.build_input_stream(
            config,
            move |data: &[T], _| {
                let bytes = bytemuck::cast_slice(data);
                prod.push_slice(bytes);

                cpal_tx.send(CpalStats {
                    requested_sample_length: data.len(),
                }).unwrap();
            },
            |err| eprintln!("Stream error: {}", err),
            None,
        )?;

        let udp_loop = std::thread::spawn(move || {
            loop {
                if cons.is_full() {
                    let mut buf: Box<[T]> =
                        vec![T::default(); buf_size as usize].into_boxed_slice();

                    let occupied_buffer = cons.occupied_len();

                    cons.pop_slice(&mut buf);

                    let packet: &[u8] = bytemuck::cast_slice(&buf);
                    //dbg!(packet);
                    let _ = socket.send(packet);

                    udp_tx.send(UdpStats {
                        sent: packet.len(),
                        occupied_buffer,
                    }).unwrap();
                }
            }
        });

        Ok((stream, udp_loop))
    }
}
