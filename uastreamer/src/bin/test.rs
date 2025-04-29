use bytemuck::Pod;
use cpal::{traits::DeviceTrait, BuildStreamError, InputCallbackInfo, OutputCallbackInfo, Sample, Stream};
use ringbuf::{HeapCons, HeapProd, traits::Producer};
use std::{
    fmt::Debug,
    sync::{mpsc::Sender, Arc, Mutex},
};
use uastreamer::{
    components::streamer::{CpalStats, Direction}, create_wav_writer, splitter::{ChannelMerger, ChannelSplitter}, streamer_config::StreamerConfig, write_debug, DebugWavWriter
};

trait CpalAudioFlow<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> {
    fn push_samples(data: &[T]) {}

    fn pop_samples(data: &mut [T]) {}

    fn get_cpal_stats_sender(&self) -> Sender<CpalStats>;

    fn construct_stream(
        &self,
        direction: Direction,
        device: &cpal::Device,
        config: StreamerConfig,
    ) -> anyhow::Result<Stream> {
        let mut writer = create_wav_writer("sender_dump".to_owned(), 1, 44100)?;

        let cpal_config = config.cpal_config.clone();
        Ok(match direction {
            Direction::Sender => {
                
                let output = self.get_producer();

                device.build_input_stream(
                    &cpal_config,
                    move |data: &[T], info| {
                        // Stream Callback
                        let mut output = output.lock().unwrap();

                        //let config = config.clone();
                        let consumed =
                            Self::process_input(&config, data, info, output.as_mut(), &mut writer);

                        /*if config.send_cpal_stats {
                            stats
                                .send(CpalStats {
                                    consumed: Some(consumed),
                                    requested: Some(data.len()),
                                    output_info: None,
                                    input_info: Some(info.clone()),
                                })
                                .unwrap();
                        }*/
                    },
                    |err| eprintln!("Stream error: {}", err),
                    None,
                )?
            }
            Direction::Receiver => {
                let cons = self.get_consumer();

                device.build_output_stream(
                    &cpal_config,
                    move |output: &mut [T], info| {
                        let mut cons = cons.lock().unwrap();

                        let consumed = Self::process_output(
                            &config,
                            output,
                            info,
                            cons.as_mut(),
                            &mut writer,
                            //cpal_tx.clone(),
                        );

                        // Sends stats about the current operation back to the front
                        /*if config.send_cpal_stats {
                            stats
                                .send(CpalStats {
                                    consumed: Some(consumed),
                                    requested: None,
                                    input_info: None,
                                    output_info: Some(info.clone()),
                                })
                                .unwrap();
                        }*/
                    },
                    |err| eprintln!("Stream error: {}", err),
                    None,
                )?
            }
        })
    }

    /// Appends the given Samples from CPAL callback to the buffer
    fn process_input(
        streamer_config: &StreamerConfig,
        data: &[T],
        info: &InputCallbackInfo,
        output: &mut HeapProd<T>,
        writer: &mut Option<DebugWavWriter>,
        //stats: Arc<Sender<CpalStats>>,
    ) -> usize {
        let mut consumed = 0;

        let splitter = ChannelSplitter::new(
            data,
            streamer_config.selected_channels.clone(),
            streamer_config.channel_count,
        )
        .unwrap();

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
        consumed
    }

    /// Writes the buffer to the specified CPAL slice
    fn process_output(
        streamer_config: &StreamerConfig,
        output: &mut [T],
        info: &OutputCallbackInfo,
        input: &mut HeapCons<T>,
        //channel_count: ChannelCount,
        //selected_channels: Vec<usize>,
        writer: &mut Option<DebugWavWriter>,
        //stats: Arc<Sender<CpalStats>>,
        //send_stats: bool,
    ) -> usize {
        let mut consumed = 0;
        // Pops the oldest element from the front and writes it to the sound buffer
        // consuming only the bytes needed

        let mut merger = ChannelMerger::new(
            input,
            &streamer_config.selected_channels,
            streamer_config.channel_count,
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

struct AppTest<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> {
    audio_buffer_prod: Arc<Mutex<HeapProd<T>>>,
    audio_buffer_cons: Arc<Mutex<HeapCons<T>>>,
}

impl<T: cpal::SizedSample + Send + Pod + Default + Debug + 'static> CpalAudioFlow<T> for AppTest<T> {
    fn get_producer(&self) -> Arc<Mutex<HeapProd<T>>> {
        self.audio_buffer_prod.clone()
    }

    fn get_consumer(&self) -> Arc<Mutex<HeapCons<T>>> {
        self.audio_buffer_cons.clone()
    }
    
    fn get_cpal_stats_sender(&self) -> Sender<CpalStats> {
        todo!()
    }
}

fn main() {

}
