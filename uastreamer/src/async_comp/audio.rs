use std::sync::Arc;

use cpal::{
    BuildStreamError, Device, InputCallbackInfo, Sample, Stream, StreamConfig,
    SupportedStreamConfig, traits::DeviceTrait,
};
use log::{debug, info, trace, warn};
use once_cell::sync::Lazy;
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Producer, Split},
};
use tokio::sync::Mutex;

use crate::splitter::ChannelSplitter;

// TODO: Replace with the master mixer
//pub static GLOBAL_MASTER_OUTPUT: Lazy<Mutex<Option<HeapProd<f32>>>> = Lazy::new(|| Mutex::new(None));
pub static GLOBAL_MASTER_OUTPUT_MIXER: Lazy<Arc<Mutex<Option<InputMixer>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));

pub static GLOBAL_MASTER_INPUT: Lazy<Mutex<Option<HeapCons<f32>>>> = Lazy::new(|| Mutex::new(None));

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
        Some(d) => d.with_sample_rate(cpal::SampleRate(requested_samplerate)),
        None => device
            .default_input_config()
            .expect("failed loading default input config"),
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
        Some(d) => d.with_sample_rate(cpal::SampleRate(requested_samplerate)),
        None => device
            .default_output_config()
            .expect("failed loading default output config"),
    }
}

pub async fn setup_master_output(
    device: Device,
    config: StreamConfig,
    //bsize: usize,
    selected_channels: Vec<u16>,
    mixer: CombinedMixer,
) -> Result<Stream, BuildStreamError> {
    //let mut master_out = GLOBAL_MASTER_OUTPUT.lock().await;
    //master_out = Some(prod);

    let mut master_mixer = GLOBAL_MASTER_OUTPUT_MIXER.lock().await;

    *master_mixer = Some(mixer.1);
    //let mut mixer = default_mixer(config.channels as usize, bsize);

    let mixer = Arc::new(std::sync::Mutex::new(mixer.0));

    device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            //trace!("Callback wants {:?} ", data.len());
            let m = mixer.clone();

            let consumed = m.lock().expect("mixer not available").mixdown(data);
            trace!("{}", consumed);
            //trace!("Consumed: {}", consumed);

            /*let mut merger =
            ChannelMerger::new(&mut cons, &selected_channels, config.channels, data.len())
                .unwrap();*/
            //cons.read(buf)
            /*for sample in data.iter_mut() {
                let s = merger.next();
                if let Some(s) = s {
                    if !cfg!(test) {
                        *sample = s;
                        //write_debug(writer, *sample);
                    } else {
                        *sample = Sample::EQUILIBRIUM;
                    }
                }

                consumed += 1;
            }*/
        },
        move |err| {},
        None,
    )
}

pub async fn setup_master_input(
    device: Device,
    config: &StreamConfig,
    bsize: usize,
    selected_channels: Vec<usize>,
) -> Result<Stream, BuildStreamError> {
    let rbuf = HeapRb::<f32>::new(bsize * config.channels as usize);
    let (mut prod, cons) = rbuf.split();

    let mut master_in = GLOBAL_MASTER_INPUT.lock().await;
    *master_in = Some(cons);

    let chcount = config.channels;

    device.build_input_stream(
        &config,
        move |data: &[f32], _: &InputCallbackInfo| {
            let splitter = ChannelSplitter::new(
                data,
                //streamer_config.selected_channels.clone(),
                selected_channels.clone(),
                chcount,
            )
            .unwrap();

            let mut dropped = 0;
            // Iterate through the input buffer and save data
            for s in splitter {
                if s.on_selected_channel {
                    if !cfg!(test) {
                        if let Ok(()) = prod.try_push(*s.sample) {
                            //write_debug(writer, *s.sample);
                        } else {
                            dropped += 1;
                            //udp_urge_channel.send(true).unwrap();

                            // Urge the UDP thread to send the buffer immediately
                            //udp_urge_channel.send(true).unwrap();

                            // drop remaining samples
                            //break;
                            //return consumed
                        }
                        //output.try_push(*s.sample).unwrap();
                    } else {
                        prod.try_push(Sample::EQUILIBRIUM).unwrap();
                    }
                }
            }
            if dropped > 0 {
                warn!("OVERFLOW - Dropped {} Samples", dropped);
            }
            //udp_urge_channel.send(true).unwrap();
        },
        move |err| {},
        None,
    )
}

pub struct OutputMixer {
    channel_count: MasterOutputChannelCount,

    buffer_size: usize,
    outputs: Vec<HeapCons<f32>>,
}

#[derive(Clone)]
pub struct InputMixer {
    inputs: Vec<Arc<Mutex<HeapProd<f32>>>>,
}

impl InputMixer {
    pub fn get_channel(&mut self, channel: usize) -> Arc<Mutex<HeapProd<f32>>> {
        self.inputs[channel].clone()
        
        //.expect("channel not found")
    }

    /*fn get_stereo_channel(&mut self, l_channel: usize, r_channel: usize) -> PairedMixer {
        let mut left_channel = self.get_channel(l_channel);
        let mut right_channel = self.get_channel(r_channel);

        PairedMixer::Stereo(left_channel, right_channel)
    }*/
}

type SharedInputMixer = Arc<Mutex<InputMixer>>;

pub type CombinedMixer = (OutputMixer, InputMixer);

type MasterOutputChannelCount = usize;

pub type RawInputChannel = Arc<Mutex<HeapProd<f32>>>;

enum PairedMixer {
    Mono(HeapProd<f32>),
    Stereo(HeapProd<f32>, HeapProd<f32>),
}

impl OutputMixer {
    fn get_channel_output_buffer(
        &mut self,
        channel: MasterOutputChannelCount,
    ) -> &mut HeapCons<f32> {
        self.outputs.get_mut(channel).expect("channel not found")
    }

    fn mixdown(&mut self, output_buffer: &mut [f32]) -> usize {
        let mut ch = 0;

        let mut consumed = 0;
        for o in output_buffer.iter_mut() {
            let c = self.get_channel_output_buffer(ch);

            *o = c.try_pop().unwrap_or(Sample::EQUILIBRIUM);
            consumed += 1;

            ch = (ch + 1) % self.channel_count;
        }

        consumed
    }
}

pub fn default_mixer(chcount: MasterOutputChannelCount, bufsize_per_channel: usize) -> CombinedMixer {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    for _ in 0..chcount {
        let buf = ringbuf::HeapRb::<f32>::new(bufsize_per_channel);

        let (prod, cons) = buf.split();
        inputs.push(Arc::new(Mutex::new(prod)));
        outputs.push(cons);
    }

    debug!(
        "Creating Mixer with {} Channels and bufsize of {} bytes",
        chcount, bufsize_per_channel
    );
    (
        OutputMixer {
            channel_count: chcount,
            buffer_size: bufsize_per_channel,
            //inputs,
            outputs,
        },
        InputMixer { inputs },
    )
}

#[cfg(test)]
mod tests {
    use ringbuf::traits::Producer;

    use crate::async_comp::audio::default_mixer;

    #[tokio::test]
    async fn test_mixer() {
        let (mut output, mut input) = default_mixer(2, 8);

        for (ch, c) in input.inputs.iter_mut().enumerate() {
            c.lock().await.try_push((ch + 1) as f32).unwrap();
        }

        let mut master_buf = vec![0.0f32; 16];
        output.mixdown(&mut master_buf);

        assert_eq!(&master_buf[..4], vec![1.0, 2.0, 0.0, 0.0]);
    }
}
