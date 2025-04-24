use std::{
    fs::File,
    io::BufWriter,
    net::{self, UdpSocket},
    sync::{
        Arc,
        mpsc::{Receiver, Sender, channel},
    },
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

/// Stats which get sent after each UDP Event
#[derive(Default)]
pub struct UdpStats {
    pub sent: Option<usize>,
    pub received: Option<usize>,
    pub occupied_buffer: usize,
}

/// Stats which get sent during each CPAL Callback Invocation after the main action is done
#[derive(Default)]
pub struct CpalStats {
    //pub requested_sample_length: usize,
    pub consumed: Option<usize>,
    pub requested: Option<usize>,
    pub input_info: Option<InputCallbackInfo>,
    pub output_info: Option<OutputCallbackInfo>,
}

/// Defines the behaivior of the stream
/// 
/// Sender: Captures from an audio input stream and sends it over the network
/// Receiver: Receives from the network and outputs it to a audio output stream
#[derive(Debug)]
pub enum Direction {
    Sender,
    Receiver,
}

/// Trait that defines the sender/receiver adapter.
///
/// The Sender Adapter simply copies the input from the specified device to a ringbuffer and sends it over the network
/// The Receiver Adapter receives the data and outputs it to the specified device
pub trait StreamComponent {
    fn construct<T: cpal::SizedSample + Send + Pod + Default + hound::Sample + 'static>(
        direction: Direction,
        port: u16,
        target: net::Ipv4Addr,
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        buf_size: usize,
    ) -> anyhow::Result<Box<Self>>;

    /// Appends the given Samples from CPAL callback to the buffer
    fn process_input<T: cpal::SizedSample + Send + Pod + Default + hound::Sample + 'static>(
        data: &[T],
        info: &InputCallbackInfo,
        output: &mut HeapProd<T>,
        stats: Arc<Sender<CpalStats>>,
    ) -> usize {
        let bytes = bytemuck::cast_slice(data);
        let appended = output.push_slice(bytes);

        stats
            .send(CpalStats {
                consumed: None,
                requested: Some(data.len()),
                output_info: None,
                input_info: Some(info.clone()),
            })
            .unwrap();

        appended
    }

    /// Writes the buffer to the specified CPAL slice
    fn process_output<T: cpal::SizedSample + Send + Pod + Default + hound::Sample + 'static>(
        output: &mut [T],
        info: &OutputCallbackInfo,
        input: &mut HeapCons<T>,
        writer: &mut Option<WavWriter<BufWriter<File>>>,
        stats: Arc<Sender<CpalStats>>,
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

        stats
            .send(CpalStats {
                consumed: Some(consumed),
                requested: None,
                input_info: None,
                output_info: Some(info.clone()),
            })
            .unwrap();

        consumed
    }

    /// Entry Point for the UDP Buffer Sender.
    /// Sends the buffer when it is full
    fn udp_sender_loop<T: cpal::SizedSample + Send + Pod + Default + hound::Sample + 'static>(
        socket: UdpSocket,
        buffer_consumer: &mut HeapCons<T>,
        stats: Sender<UdpStats>,
    ) {
        loop {
            if buffer_consumer.is_full() {
                // TODO Check if this might slow down communication
                let mut network_buffer: Box<[T]> =
                    vec![T::default(); buffer_consumer.capacity().into()].into_boxed_slice();

                // get buffer size before changes
                let pre_occupied_buffer = buffer_consumer.occupied_len();
                
                // Place the network buffer onto the stack
                buffer_consumer.pop_slice(&mut network_buffer);

                let post_occupied_buffer = buffer_consumer.occupied_len();

                let udp_packet: &[u8] = bytemuck::cast_slice(&network_buffer);
                //dbg!(packet);
                let _ = socket.send(udp_packet);

                // Send statistics to the channel
                stats
                    .send(UdpStats {
                        sent: Some(udp_packet.len()),
                        received: None,
                        occupied_buffer: pre_occupied_buffer,
                    })
                    .unwrap();
            }
        }
    }

    /// Entry Point for the UDP Receiver Loop
    fn udp_receiver_loop<T: cpal::SizedSample + Send + Pod + Default + hound::Sample + 'static>(
        socket: UdpSocket,
        buffer_producer: &mut HeapProd<T>,
        stats: Sender<UdpStats>,
    ) {
        // How big is one byte?
        let byte_size = size_of::<T>();

        loop {
            let cap: usize = buffer_producer.capacity().into();

            // create the temporary network buffer needed to capture the network samples
            let mut temp_network_buffer: Box<[u8]> = vec![0u8; cap * byte_size].into_boxed_slice();

            // Receive from the Network
            match socket.recv(&mut temp_network_buffer) {
                Ok(received) => {
                    // Convert the buffered network samples to the specified sample format
                    let converted_samples: &[T] = bytemuck::cast_slice(&temp_network_buffer);
                    
                    let pre_occupied_buffer = buffer_producer.occupied_len();
                    
                    // Transfer Samples bytewise
                    for &sample in converted_samples {
                        // TODO implement fell-behind logic here
                        let _ = buffer_producer.try_push(sample);
                    }

                    let post_occupied_buffer = buffer_producer.occupied_len();


                    // Send Statistics about the current operation to the stats channel
                    stats
                        .send(UdpStats {
                            sent: None,
                            received: Some(received),
                            occupied_buffer: pre_occupied_buffer,
                        })
                        .unwrap();
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

/// Struct that holds the Streamer.
/// See [StreamComponent] for more information about how the streamer works
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
        // TODO Implement sample conversion for debug hound writer
        Ok(match format {
            cpal::SampleFormat::I16 => {
                Self::construct::<i16>(direction, port, target, device, config, buf_size)
            }
            //cpal::SampleFormat::U16 => Self::construct::<u16>(direction, port, target, device, config, buf_size),
            cpal::SampleFormat::I8 => {
                Self::construct::<i8>(direction, port, target, device, config, buf_size)
            }
            cpal::SampleFormat::I32 => {
                Self::construct::<i32>(direction, port, target, device, config, buf_size)
            }
            //cpal::SampleFormat::I64 => Self::construct::<i64>(direction, port, target, device, config, buf_size),
            //cpal::SampleFormat::U8 => Self::construct::<u8>(direction, port, target, device, config, buf_size),
            //cpal::SampleFormat::U32 => Self::construct::<u32>(direction, port, target, device, config, buf_size),
            //cpal::SampleFormat::U64 => Self::construct::<u64>(direction, port, target, device, config, buf_size),
            //cpal::SampleFormat::F64 => Self::construct::<f64>(direction, port, target, device, config, buf_size),
            cpal::SampleFormat::F32 => {
                Self::construct::<f32>(direction, port, target, device, config, buf_size)
            }
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
                            _ = Self::process_output(
                                output,
                                info,
                                &mut cons,
                                &mut writer,
                                cpal_tx.clone(),
                            );
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
            cpal_stats,
        }))
    }

    fn get_bufer_size(&self) -> usize {
        todo!()
    }

    fn set_bufer_size(&self, size: usize) {
        todo!()
    }
}
