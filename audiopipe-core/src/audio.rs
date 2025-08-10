use std::sync::Arc;

use cpal::{
    BuildStreamError, Device, InputCallbackInfo, Sample, Stream, StreamConfig,
    SupportedStreamConfig, traits::DeviceTrait,
};
use log::{debug, trace, warn};
use once_cell::sync::Lazy;
use ringbuf::{
    HeapRb,
    traits::{Producer, Split},
};
use tokio::sync::Mutex;

use crate::{mixer::{default_client_mixer, AsyncMixerInputEnd, AsyncMixerOutputEnd, ServerMixer}, splitter::ChannelSplitter};

/// The global output mixer used by the receiver
pub static GLOBAL_MASTER_OUTPUT_MIXER: Lazy<Arc<Mutex<Option<AsyncMixerInputEnd>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// The global input mixer used by the sender
pub static GLOBAL_MASTER_INPUT_MIXER: Lazy<Arc<Mutex<Option<AsyncMixerOutputEnd>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

//pub static GLOBAL_MASTER_INPUT: Lazy<Mutex<Option<HeapCons<f32>>>> = Lazy::new(|| Mutex::new(None));

/// returns a suitable cpal config that tries to be as close to the requested specification as possible
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
    //selected_channels: Vec<u16>,
    mixer: ServerMixer,
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

            let mixer = m.lock().expect("mixer not available");
            //.mixdown(data);
        let consumed = 0;
            trace!("Consumed {} bytes", consumed);
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

    //let mut master_in = GLOBAL_MASTER_INPUT.lock().await;
    //*master_in = Some(cons);

    let mixer = default_client_mixer(selected_channels.len(), bsize);

    let mut master_mixer = GLOBAL_MASTER_INPUT_MIXER.lock().await;
    *master_mixer = Some(mixer.0);

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



