use bytemuck::Pod;
use core::net;
use cpal::{
    traits::DeviceTrait, BuildStreamError, ChannelCount, InputCallbackInfo, OutputCallbackInfo, Sample, Stream, StreamConfig
};
use ringbuf::{
    HeapCons, HeapProd,
    traits::{Observer, Producer},
};
use std::{
    fmt::Debug,
    sync::{Arc, Mutex, mpsc::Sender},
};

use crate::{
    DebugWavWriter, Direction, create_wav_writer,
    splitter::{ChannelMerger, ChannelSplitter},
    streamer_config::StreamerConfig,
    write_debug,
};

/// Stats which get sent during each CPAL Callback Invocation after the main action is done
#[derive(Default)]
pub struct CpalStats {
    //pub requested_sample_length: usize,
    pub consumed: Option<usize>,
    pub requested: Option<usize>,
    pub input_info: Option<InputCallbackInfo>,
    pub output_info: Option<OutputCallbackInfo>,
}

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
    ) -> anyhow::Result<Stream> {
        //let stats = self.get_cpal_stats_sender();

        Ok(match direction {
            Direction::Sender => {
                //                let output = self.get_producer();
                let mut writer = create_wav_writer("sender_dump".to_owned(), 1, 44100)?;

                device.build_input_stream(
                    &cpal_config,
                    move |data: &[T], info| {
                        // Stream Callback
                        let mut prod = prod.lock().unwrap();

                        //let config = config.clone();
                        let channel_count = cpal_config.channels;
                        let consumed =
                            Self::process_input(data, info, prod.as_mut(), &mut writer, config.selected_channels.clone(), channel_count);

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
                let mut writer = create_wav_writer("receiver_dump".to_owned(), 1, 44100)?;

                device.build_output_stream(
                    &cpal_config,
                    move |output: &mut [T], info| {
                        let mut cons = cons.lock().unwrap();

                        let channel_count = cpal_config.channels;
                        let selected_channels = config.selected_channels.clone();

                        let consumed = Self::process_output(
                            //&config,
                            output,
                            info,
                            cons.as_mut(),
                            &mut writer,
                            channel_count,
                            selected_channels
                            //cpal_tx.clone(),
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
        info: &InputCallbackInfo,
        output: &mut HeapProd<T>,
        writer: &mut Option<DebugWavWriter>,
        selected_channels: Vec<usize>,
        channel_count: u16
        //stats: Arc<Sender<CpalStats>>,
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

        // Iterate through the input buffer and save data
        for s in splitter {
            if s.on_selected_channel {
                if !cfg!(test) {
                    output.try_push(*s.sample).unwrap();
                } else {
                    output.try_push(Sample::EQUILIBRIUM).unwrap();
                }

                // If the program runs in debug mode, the debug wav writer becomes available

                if let Some(writer) = writer {
                    //writer.write_sample(*s.sample).unwrap();
                    
                }
            }

            consumed += 1;
        }
        //println!("buffer after splitter: {}", output.occupied_len());
        consumed
    }

    /// Writes the buffer to the specified CPAL slice
    fn process_output(
        //streamer_config: &StreamerConfig,
        output: &mut [T],
        info: &OutputCallbackInfo,
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

        let mut merger = ChannelMerger::new(
            input,
            &selected_channels,
            channel_count,
            output.len(),
        )
        .unwrap();

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

        consumed
    }

    fn get_producer(&self) -> Arc<Mutex<HeapProd<T>>>;
    fn get_consumer(&self) -> Arc<Mutex<HeapCons<T>>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x_test_transfer() {}
}
