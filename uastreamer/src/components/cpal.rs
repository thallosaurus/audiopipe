use bytemuck::Pod;
use cpal::{
    traits::DeviceTrait, ChannelCount, InputCallbackInfo, OutputCallbackInfo, Sample, Stream, StreamConfig, SupportedStreamConfig
};
use log::{debug, warn};
use ringbuf::{
    HeapCons, HeapProd,
    traits::Producer,
};
use std::{
    fmt::Debug,
    sync::{Arc, Mutex, mpsc::Sender},
};

use crate::{
    DebugWavWriter, Direction,
    config::StreamerConfig,
    create_wav_writer,
    splitter::{ChannelMerger, ChannelSplitter},
    write_debug,
};

#[deprecated]
pub enum CpalError {}

#[deprecated]
pub enum CpalStatus {
    DidEnd,
}

/// Stats which get sent during each CPAL Callback Invocation after the main action is done
#[derive(Default)]
#[deprecated]
pub struct CpalStats {
    //pub requested_sample_length: usize,
    pub consumed: Option<usize>,
    pub requested: Option<usize>,
    pub input_info: Option<InputCallbackInfo>,
    pub output_info: Option<OutputCallbackInfo>,
}

pub fn select_input_device_config(
    device: &cpal::Device,
    requested_bufsize: u32,
    requested_samplerate: u32,
    chcount: usize,
) -> SupportedStreamConfig {
    let devs = device.supported_input_configs().unwrap().find(|x| {
        let bsize = *x.buffer_size();
        let bsize_match = match bsize {
            cpal::SupportedBufferSize::Range { min, max } => {
                min <= requested_bufsize && requested_bufsize <= max
            }
            cpal::SupportedBufferSize::Unknown => false,
        };
        
        debug!(
            "min_sample_rate: {}, max_sample_rate: {}",
            x.min_sample_rate().0,
            x.max_sample_rate().0
        );

        return x.min_sample_rate().0 <= requested_samplerate
            && x.max_sample_rate().0 >= requested_samplerate
            && x.channels() >= chcount as u16;
    });

    println!("{:?}", devs);

    match devs {
        Some(d) => d
            .with_sample_rate(cpal::SampleRate(requested_samplerate)),
        None => device
            .default_input_config()
            .expect("failed loading default input config")
    }
}

pub fn select_output_device_config(
    device: &cpal::Device,
    requested_bufsize: u32,
    requested_samplerate: u32,
    chcount: usize,
) -> SupportedStreamConfig {
    let devs = device.supported_output_configs().unwrap().find(|x| {
        let bsize = *x.buffer_size();
        let bsize_match = match bsize {
            cpal::SupportedBufferSize::Range { min, max } => {
                min <= requested_bufsize && requested_bufsize <= max
            }
            cpal::SupportedBufferSize::Unknown => false,
        };

        debug!(
            "min_sample_rate: {}, max_sample_rate: {}",
            x.min_sample_rate().0,
            x.max_sample_rate().0
        );

        return x.min_sample_rate().0 <= requested_samplerate
            && x.max_sample_rate().0 >= requested_samplerate
            && x.channels() >= chcount as u16;
    });

    println!("{:?}", devs);

    match devs {
        Some(d) => d
            .with_sample_rate(cpal::SampleRate(requested_samplerate)),
        None => device
            .default_output_config()
            .expect("failed loading default output config"),
    }
}

#[deprecated]
pub trait CpalAudioFlow<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> {
    fn get_cpal_stats_sender(&self) -> Sender<CpalStats>;

    // Static because this is the entry point from the new thread
    fn construct_stream(
        direction: Direction,
        device: &cpal::Device,
        cpal_config: StreamConfig,
        config: StreamerConfig,
        stats: Sender<CpalStats>,
        prod: Arc<Mutex<HeapProd<T>>>,
        cons: Arc<Mutex<HeapCons<T>>>,
        cpal_channel_tx: Sender<bool>,
    ) -> anyhow::Result<Stream> {
        //let stats = self.get_cpal_stats_sender();

        Ok(match direction {
            Direction::Sender => {
                let mut writer = create_wav_writer(
                    "sender_dump".to_owned(),
                    config.selected_channels.len() as u16,
                    cpal_config.sample_rate.0,
                )?;

                device.build_input_stream(
                    &cpal_config,
                    move |data: &[T], info| {
                        // Stream Callback
                        let mut prod = prod.lock().unwrap();

                        //let config = config.clone();
                        let channel_count = cpal_config.channels;
                        let consumed = Self::process_input(
                            data,
                            Some(info),
                            prod.as_mut(),
                            &mut writer,
                            config.selected_channels.clone(),
                            channel_count,
                            cpal_channel_tx.clone(),
                        );

                        if config.send_stats {
                            stats
                                .send(CpalStats {
                                    consumed: Some(consumed),
                                    requested: Some(data.len()),
                                    output_info: None,
                                    input_info: Some(info.clone()),
                                })
                                .unwrap();
                        }
                    },
                    |err| eprintln!("Stream error: {}", err),
                    None,
                )?
            }
            Direction::Receiver => {
                //let cons = self.get_consumer();
                let mut writer = create_wav_writer(
                    "receiver_dump".to_owned(),
                    config.selected_channels.len() as u16,
                    cpal_config.sample_rate.0,
                )?;

                device.build_output_stream(
                    &cpal_config,
                    move |output: &mut [T], info| {
                        let mut cons = cons.lock().unwrap();

                        let channel_count = cpal_config.channels;
                        let selected_channels = config.selected_channels.clone();

                        let consumed = Self::process_output(
                            output,
                            Some(info),
                            cons.as_mut(),
                            &mut writer,
                            channel_count,
                            selected_channels, //cpal_tx.clone(),
                        );

                        // Sends stats about the current operation back to the front
                        if config.send_stats {
                            stats
                                .send(CpalStats {
                                    consumed: Some(consumed),
                                    requested: None,
                                    input_info: None,
                                    output_info: Some(info.clone()),
                                })
                                .unwrap();
                        }
                    },
                    |err| eprintln!("Stream error: {}", err),
                    None,
                )?
            }
        })
    }

    /// Appends the given Samples from CPAL callback to the buffer
    fn process_input(
        //streamer_config: &StreamerConfig,
        data: &[T],
        _info: Option<&InputCallbackInfo>,
        output: &mut HeapProd<T>,
        writer: &mut Option<DebugWavWriter>,
        selected_channels: Vec<usize>,
        channel_count: u16, //stats: Arc<Sender<CpalStats>>,
        udp_urge_channel: Sender<bool>,
    ) -> usize {
        let mut consumed = 0;

        let splitter = ChannelSplitter::new(
            data,
            //streamer_config.selected_channels.clone(),
            selected_channels,
            channel_count,
        )
        .unwrap();

        //println!("Buffer Size inside cpal: {}, CPAL Len: {}", output.capacity(), data.len());

        let mut dropped = 0;
        // Iterate through the input buffer and save data
        for s in splitter {
            if s.on_selected_channel {
                if !cfg!(test) {
                    if let Ok(()) = output.try_push(*s.sample) {
                        write_debug(writer, *s.sample);
                    } else {
                        dropped += 1;
                        udp_urge_channel.send(true).unwrap();

                        // Urge the UDP thread to send the buffer immediately
                        //udp_urge_channel.send(true).unwrap();

                        // drop remaining samples
                        //break;
                        //return consumed
                    }
                    //output.try_push(*s.sample).unwrap();
                } else {
                    output.try_push(Sample::EQUILIBRIUM).unwrap();
                }
            }

            consumed += 1;
        }
        if dropped > 0 {
            warn!("OVERFLOW - Dropped {} Samples", dropped);
        }
        udp_urge_channel.send(true).unwrap();

        consumed
    }

    /// Writes the buffer to the specified CPAL slice
    fn process_output(
        //streamer_config: &StreamerConfig,
        output: &mut [T],
        _info: Option<&OutputCallbackInfo>,
        input: &mut HeapCons<T>,
        writer: &mut Option<DebugWavWriter>,
        channel_count: ChannelCount,
        selected_channels: Vec<usize>,
        //stats: Arc<Sender<CpalStats>>,
        //send_stats: bool,
    ) -> usize {
        let mut consumed = 0;
        // Pops the oldest element from the front and writes it to the sound buffer
        // consuming only the bytes needed

        let mut merger =
            ChannelMerger::new(input, &selected_channels, channel_count, output.len()).unwrap();

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

            consumed += 1;
        }

        consumed
    }

    fn get_producer(&self) -> Arc<Mutex<HeapProd<T>>>;
    fn get_consumer(&self) -> Arc<Mutex<HeapCons<T>>>;
}

#[cfg(test)]
mod tests {
    const _MONO_WAV: &'static [u8] = include_bytes!("../../assets/mono-440hz.wav");

    use std::sync::{
        Arc, Mutex,
        mpsc::{Sender, channel},
    };

    use ringbuf::{HeapCons, HeapProd, HeapRb, traits::Split};

    use super::{CpalAudioFlow, CpalStats};

    struct CpalAudioDebugAdapter {
        prod: Arc<Mutex<HeapProd<f32>>>,
        cons: Arc<Mutex<HeapCons<f32>>>,
        _sender: Sender<CpalStats>,
    }

    impl CpalAudioDebugAdapter {
        pub fn new(buffer_size: usize) -> Self {
            let buffer = HeapRb::<f32>::new(buffer_size);

            let (prod, cons) = buffer.split();

            let (_sender, _) = channel();

            Self {
                prod: Arc::new(Mutex::new(prod)),
                cons: Arc::new(Mutex::new(cons)),
                _sender,
            }
        }
    }

    impl CpalAudioFlow<f32> for CpalAudioDebugAdapter {
        fn get_cpal_stats_sender(&self) -> std::sync::mpsc::Sender<super::CpalStats> {
            todo!()
        }

        fn get_producer(&self) -> Arc<Mutex<HeapProd<f32>>> {
            self.prod.clone()
        }

        fn get_consumer(&self) -> Arc<Mutex<HeapCons<f32>>> {
            self.cons.clone()
        }
    }
}
