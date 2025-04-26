use std::{
    fmt::Debug,
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
    ChannelCount, InputCallbackInfo, OutputCallbackInfo, Sample, Stream,
    traits::{DeviceTrait, StreamTrait},
};
use hound::WavWriter;
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};

use crate::{
    create_wav_writer, splitter::{ChannelMerger, ChannelSplitter}, write_debug, DebugWavWriter
};

/// Stats which get sent after each UDP Event
#[derive(Default)]
pub struct UdpStats {
    pub sent: Option<usize>,
    pub received: Option<usize>,
    pub pre_occupied_buffer: usize,
    pub post_occupied_buffer: usize,
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

pub struct StreamerConfig {
    stream_config: cpal::StreamConfig,
    buffer_size: usize,
    channel_count: ChannelCount,
    send_network_stats: bool,
    send_cpal_stats: bool,
}

/// Trait that defines the sender/receiver adapter.
///
/// The Sender Adapter simply copies the input from the specified device to a ringbuffer and sends it over the network
/// The Receiver Adapter receives the data and outputs it to the specified device
pub trait StreamComponent {
    fn construct<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static>(
        direction: Direction,
        port: u16,
        target: net::Ipv4Addr,
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        selected_channels: Vec<usize>,
        buf_size: usize,
        send_stats: bool,
    ) -> anyhow::Result<Box<Self>>;

    /// Appends the given Samples from CPAL callback to the buffer
    fn process_input<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static>(
        data: &[T],
        info: &InputCallbackInfo,
        output: &mut HeapProd<T>,
        channel_count: ChannelCount,
        selected_channels: Vec<usize>,
        writer: &mut Option<DebugWavWriter>,
        stats: Arc<Sender<CpalStats>>,
        send_stats: bool,
    ) -> usize {
        //let bytes = bytemuck::cast_slice(data);
        //let appended = output.push_slice(bytes);

        let mut consumed = 0;

        let splitter = ChannelSplitter::new(data, selected_channels, channel_count);

        // Iterate through the input buffer and save data
        // TODO NOTE: The size of the slice is buffer_size * channelCount,
        // so you have to slice the data somehow
        for s in splitter {
            if s.on_selected_channel {
                if !cfg!(test) {
                    output.try_push(*s.sample).unwrap();
                } else {
                    output.try_push(Sample::EQUILIBRIUM).unwrap();
                }
            }
            // If the program runs in debug mode, the debug wav writer becomes available
            //#[cfg(debug_assertions)]
            /*if let Some(writer) = writer {
                writer.write_sample(*s.sample).unwrap();
            }*/

            consumed += 1;
        }

        if send_stats {
            stats
                .send(CpalStats {
                    consumed: Some(consumed),
                    requested: Some(data.len()),
                    output_info: None,
                    input_info: Some(info.clone()),
                })
                .unwrap();
        }
        consumed
    }

    /// Writes the buffer to the specified CPAL slice
    fn process_output<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static>(
        output: &mut [T],
        info: &OutputCallbackInfo,
        input: &mut HeapCons<T>,
        channel_count: ChannelCount,
        selected_channels: Vec<usize>,
        writer: &mut Option<DebugWavWriter>,
        stats: Arc<Sender<CpalStats>>,
        send_stats: bool,
    ) -> usize {
        let mut consumed = 0;
        // Pops the oldest element from the front and writes it to the sound buffer
        // consuming only the bytes needed

        let mut merger = ChannelMerger::new(input, selected_channels, channel_count, output.len());

        for sample in output.iter_mut() {
            let s = merger.next();
            if let Some(s) = s {
                if !cfg!(test) {
                    *sample = s;
                    write_debug(writer, *sample);

                } else {
                    *sample = Sample::EQUILIBRIUM;
                }
            }

            // If the program runs in debug mode, the debug wav writer becomes available
            /*#[cfg(debug_assertions)]
            if let Some(writer) = writer {
                writer.write_sample(*sample).unwrap();
            }*/

            consumed += 1;
        }

        // Sends stats about the current operation back to the front
        if send_stats {
            stats
                .send(CpalStats {
                    consumed: Some(consumed),
                    requested: None,
                    input_info: None,
                    output_info: Some(info.clone()),
                })
                .unwrap();
        }

        consumed
    }

    /// Entry Point for the UDP Buffer Sender.
    /// Sends the buffer when it is full
    fn udp_sender_loop<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static>(
        socket: UdpSocket,
        buffer_consumer: &mut HeapCons<T>,
        stats: Sender<UdpStats>,
        send_stats: bool,
    ) {
        loop {
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
                if send_stats {
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
    fn udp_receiver_loop<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static>(
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
                            pre_occupied_buffer,
                            post_occupied_buffer,
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
        selected_channels: Vec<usize>,
        buf_size: usize,
        send_stats: bool,
    ) -> anyhow::Result<Box<Self>> {
        // TODO Implement sample conversion for debug hound writer
        Ok(match format {
            cpal::SampleFormat::I16 => Self::construct::<i16>(
                direction, port, target, device, config, selected_channels, buf_size, send_stats,
            ),
            cpal::SampleFormat::U16 => Self::construct::<u16>(
                direction, port, target, device, config, selected_channels, buf_size, send_stats,
            ),
            cpal::SampleFormat::I8 => Self::construct::<i8>(
                direction, port, target, device, config, selected_channels, buf_size, send_stats,
            ),
            cpal::SampleFormat::I32 => Self::construct::<i32>(
                direction, port, target, device, config, selected_channels, buf_size, send_stats,
            ),
            cpal::SampleFormat::I64 => Self::construct::<i64>(
                direction, port, target, device, config, selected_channels, buf_size, send_stats,
            ),
            cpal::SampleFormat::U8 => Self::construct::<u8>(
                direction, port, target, device, config, selected_channels, buf_size, send_stats,
            ),
            cpal::SampleFormat::U32 => Self::construct::<u32>(
                direction, port, target, device, config, selected_channels, buf_size, send_stats,
            ),
            cpal::SampleFormat::U64 => Self::construct::<u64>(
                direction, port, target, device, config, selected_channels, buf_size, send_stats,
            ),
            cpal::SampleFormat::F64 => Self::construct::<f64>(
                direction, port, target, device, config, selected_channels, buf_size, send_stats,
            ),
            cpal::SampleFormat::F32 => Self::construct::<f32>(
                direction, port, target, device, config, selected_channels, buf_size, send_stats,
            ),
            _ => panic!("Unsupported Sample Format: {:?}", format),
        }?)
    }
}

impl StreamComponent for Streamer {
    fn construct<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static>(
        direction: Direction,
        port: u16,
        target: net::Ipv4Addr,
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        selected_channels: Vec<usize>,
        buf_size: usize,
        send_stats: bool,
    ) -> Result<Box<Self>, anyhow::Error> {
        let buf = HeapRb::<T>::new((buf_size as usize) * 2);
        let (mut prod, mut cons) = buf.split();

        let (net_tx, net_stats) = channel::<UdpStats>();
        let (cpal_tx, cpal_stats) = channel::<CpalStats>();

        let cpal_arc = Arc::new(cpal_tx);

        // Start CPAL Stream and also start UDP Thread
        let (_stream, _udp_loop) = match direction {
            Direction::Sender => {
                let socket = UdpSocket::bind("0.0.0.0:0")?;
                socket.connect(format!("{}:{}", target, port))?;

                #[cfg(debug_assertions)]
                let mut writer = create_wav_writer(
                    "sender_dump".to_owned(),
                    1,
                    44100,
                )?;

                let channel_count = config.channels.clone();
                let cpal_tx = cpal_arc.clone();
                (
                    device.build_input_stream(
                        config,
                        move |data: &[T], c| {
                            let sel = selected_channels.clone();
                            _ = Self::process_input(
                                data,
                                c,
                                &mut prod,
                                // It is neccessary to only hand over the specific properties
                                // or otherwise it will complain
                                channel_count,
                                sel,

                                &mut writer,
                                cpal_tx.clone(),
                                send_stats,
                            );
                        },
                        |err| eprintln!("Stream error: {}", err),
                        None,
                    )?,
                    std::thread::spawn(move || {
                        Self::udp_sender_loop(socket, &mut cons, net_tx, send_stats);
                    }),
                )
            }
            Direction::Receiver => {
                let socket = UdpSocket::bind(("0.0.0.0", port)).expect("Failed to bind UDP socket");

                let mut writer = create_wav_writer(
                    "receiver_dump".to_owned(),
                    1,
                    44100,
                )?;

                let cpal_tx = cpal_arc.clone();
                let channel_count = config.channels.clone();
                (
                    device.build_output_stream(
                        config.into(),
                        move |output: &mut [T], info| {
                            let sel = selected_channels.clone();
                            _ = Self::process_output(
                                output,
                                info,
                                &mut cons,
                                channel_count,
                                sel,
                                &mut writer,
                                cpal_tx.clone(),
                                send_stats,
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
