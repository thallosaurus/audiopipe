use std::{
    fs::File,
    io::BufWriter,
    net::{self, UdpSocket},
    sync::{mpsc::{channel, Receiver, Sender}, Arc},
    thread::JoinHandle,
};

use bytemuck::Pod;
use cpal::{
    InputCallbackInfo, OutputCallbackInfo, Sample, Stream,
    traits::{DeviceTrait, StreamTrait},
};
use hound::WavWriter;
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};

use crate::create_wav_writer;

#[derive(Default)]
pub struct UdpStats {
    pub sent: Option<usize>,
    pub received: Option<usize>,
    pub occupied_buffer: usize,
}

#[derive(Default)]
pub struct CpalStats {
    //pub requested_sample_length: usize,
    pub consumed: Option<usize>,
    pub requested: Option<usize>,
    pub input_info: Option<InputCallbackInfo>,
    pub output_info: Option<OutputCallbackInfo>
}

#[derive(Debug)]
pub enum Direction {
    Sender,
    Receiver,
}

pub trait StreamComponent {
    fn construct<T: cpal::SizedSample + Send + Pod + Default + hound::Sample + 'static>(
        direction: Direction,
        port: u16,
        target: net::Ipv4Addr,
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        buf_size: usize,
    ) -> anyhow::Result<Box<Self>>;

    fn process_input<T: cpal::SizedSample + Send + Pod + Default + hound::Sample + 'static>(
        data: &[T],
        info: &InputCallbackInfo,
        output: &mut HeapProd<T>,
        stats: Arc<Sender<CpalStats>>
    ) -> usize {
        let bytes = bytemuck::cast_slice(data);
        let appended = output.push_slice(bytes);

        stats.send(CpalStats { consumed: None, requested: Some(data.len()), output_info: None, input_info: Some(info.clone()) }).unwrap();

        appended
    }
    fn process_output<T: cpal::SizedSample + Send + Pod + Default + hound::Sample + 'static>(
        output: &mut [T],
        info: &OutputCallbackInfo,
        input: &mut HeapCons<T>,
        writer: &mut Option<WavWriter<BufWriter<File>>>,
        stats: Arc<Sender<CpalStats>>
    ) -> usize {
        let mut consumed = 0;

        for sample in output.iter_mut() {
            *sample = input.try_pop().unwrap_or(Sample::EQUILIBRIUM);

            #[cfg(debug_assertions)]
            if let Some(writer) = writer {
                writer.write_sample(*sample).unwrap();
            }

            consumed += 1;
        }

        stats.send(CpalStats { consumed: Some(consumed), requested: None, input_info: None, output_info: Some(info.clone()) }).unwrap();

        consumed
    }
    fn udp_sender_loop<T: cpal::SizedSample + Send + Pod + Default + hound::Sample + 'static>(
        socket: UdpSocket,
        cons: &mut HeapCons<T>,
        stats: Sender<UdpStats>
    ) {
        loop {
            if cons.is_full() {
                let mut buf: Box<[T]> =
                    vec![T::default(); cons.capacity().into()].into_boxed_slice();

                let occupied_buffer = cons.occupied_len();

                cons.pop_slice(&mut buf);

                let packet: &[u8] = bytemuck::cast_slice(&buf);
                //dbg!(packet);
                let _ = socket.send(packet);

                stats.send(UdpStats { sent: Some(packet.len()), received: None, occupied_buffer }).unwrap();
            }
        }
    }
    fn udp_receiver_loop<T: cpal::SizedSample + Send + Pod + Default + hound::Sample + 'static>(
        socket: UdpSocket,
        prod: &mut HeapProd<T>,
        stats: Sender<UdpStats>
    ) {
        let t_size = size_of::<T>();

        let cap: usize = prod.capacity().into();

        let mut raw_buf: Box<[u8]> = vec![0u8; cap * t_size].into_boxed_slice();

        loop {
            match socket.recv(&mut raw_buf) {
                Ok(received) => {
                    let float_samples: &[T] = bytemuck::cast_slice(&raw_buf);

                    //let mut prod = producer_clone.lock().unwrap();

                    for &sample in float_samples {
                        // TODO implement fell-behind logic here
                        let _ = prod.try_push(sample);
                    }

                    let occupied_buffer = prod.occupied_len();

                    stats.send(UdpStats { sent: None, received: Some(received), occupied_buffer }).unwrap();
                }
                Err(e) => eprintln!("UDP receive error: {:?}", e),
            }
        }
    }

    fn get_bufer_size(&self) -> usize;
    fn set_bufer_size(&self, size: usize);
    fn get_network_buffer_size(&self) -> usize {
        //TODO DIRTY
        self.get_bufer_size() * 2
    }
    fn get_encoding_buffer_size(&self) -> usize {
        //TODO DIRTY
        self.get_bufer_size() * 4
    }
}

pub struct Streamer {
    _stream: Stream,
    _udp_loop: JoinHandle<()>,
    direction: Direction,
    pub net_stats: Receiver<UdpStats>,
    pub cpal_stats: Receiver<CpalStats>,
}

impl Streamer {
    pub fn from_sample_format(
        format: cpal::SampleFormat,
        direction: Direction,
        port: u16,
        target: net::Ipv4Addr,
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        buf_size: usize,
    ) -> anyhow::Result<Box<Self>> {
        Ok(match format {
            cpal::SampleFormat::I16 => Self::construct::<i16>(direction, port, target, device, config, buf_size),
            //cpal::SampleFormat::U16 => Self::construct::<u16>(direction, port, target, device, config, buf_size),
            cpal::SampleFormat::I8 => Self::construct::<i8>(direction, port, target, device, config, buf_size),
            cpal::SampleFormat::I32 => Self::construct::<i32>(direction, port, target, device, config, buf_size),
            //cpal::SampleFormat::I64 => Self::construct::<i64>(direction, port, target, device, config, buf_size),
            //cpal::SampleFormat::U8 => Self::construct::<u8>(direction, port, target, device, config, buf_size),
            //cpal::SampleFormat::U32 => Self::construct::<u32>(direction, port, target, device, config, buf_size),
            //cpal::SampleFormat::U64 => Self::construct::<u64>(direction, port, target, device, config, buf_size),
            //cpal::SampleFormat::F64 => Self::construct::<f64>(direction, port, target, device, config, buf_size),
            cpal::SampleFormat::F32 => Self::construct::<f32>(direction, port, target, device, config, buf_size),
            _ => panic!("Unsupported Sample Format: {:?}", format),
        }?)
    }
}

impl StreamComponent for Streamer {
    fn construct<T: cpal::SizedSample + Send + Pod + Default + hound::Sample + 'static>(
        direction: Direction,
        port: u16,
        target: net::Ipv4Addr,
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        buf_size: usize,
    ) -> Result<Box<Self>, anyhow::Error> {
        let buf = HeapRb::<T>::new((buf_size as usize) * 2);
        let (mut prod, mut cons) = buf.split();

        let (net_tx, net_stats) = channel::<UdpStats>();
        let (cpal_tx, cpal_stats) = channel::<CpalStats>();

        let cpal_arc = Arc::new(cpal_tx);

        let (_stream, _udp_loop) = match direction {
            Direction::Sender => {
                let socket = UdpSocket::bind("0.0.0.0:0")?;
                socket.connect(format!("{}:{}", target, port))?;

                let cpal_tx = cpal_arc.clone();
                (
                    device.build_input_stream(
                        config,
                        move |data: &[T], c| {
                            _ = Self::process_input(data, c, &mut prod, cpal_tx.clone());
                        },
                        |err| eprintln!("Stream error: {}", err),
                        None,
                    )?,
                    std::thread::spawn(move || {
                        Self::udp_sender_loop(socket, &mut cons, net_tx);
                    }),
                )
            }
            Direction::Receiver => {
                let socket = UdpSocket::bind(("0.0.0.0", port)).expect("Failed to bind UDP socket");

                #[cfg(debug_assertions)]
                let mut writer = Some(create_wav_writer(
                    "receiver_dump.wav".to_owned(),
                    1,
                    44100,
                    32,
                    hound::SampleFormat::Float,
                )?);

                #[cfg(not(debug_assertions))]
                let mut writer = None;

                let cpal_tx = cpal_arc.clone();

                (
                    device.build_output_stream(
                        config.into(),
                        move |output: &mut [T], info| {
                            _ = Self::process_output(output, info, &mut cons, &mut writer, cpal_tx.clone());
                        },
                        |err| eprintln!("Stream error: {}", err),
                        None,
                    )?,
                    std::thread::spawn(move || {
                        Self::udp_receiver_loop(socket, &mut prod, net_tx);
                    }),
                )
            }
        };

        _stream.play()?;

        Ok(Box::new(Self {
            _stream,
            _udp_loop,
            direction,
            net_stats,
            cpal_stats
        }))
    }
    
    fn get_bufer_size(&self) -> usize {
        todo!()
    }
    
    fn set_bufer_size(&self, size: usize) {
        todo!()
    }

}