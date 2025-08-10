use std::{ops::Deref, sync::Arc};

use cpal::{
    BuildStreamError, Device, InputCallbackInfo, Stream, StreamConfig,
    SupportedStreamConfig, traits::DeviceTrait,
};
use log::{debug, trace, warn};
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

use crate::mixer::{mixdown_sync, transfer_sync, AsyncMixerInputEnd, AsyncMixerOutputEnd, SyncMixerInputEnd, SyncMixerOutputEnd};

/// The global output mixer used by the receiver
pub static GLOBAL_MASTER_OUTPUT_MIXER: Lazy<Arc<Mutex<Option<AsyncMixerInputEnd>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

pub async fn set_global_master_output_mixer(mixer: AsyncMixerInputEnd) {
    let mut master_mixer = GLOBAL_MASTER_OUTPUT_MIXER.lock().await;
    *master_mixer = Some(mixer);
}

/// The global input mixer used by the sender
pub static GLOBAL_MASTER_INPUT_MIXER: Lazy<Arc<Mutex<Option<AsyncMixerOutputEnd>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

pub async fn set_global_master_input_mixer(mixer: AsyncMixerOutputEnd) {
    let mut master_mixer = GLOBAL_MASTER_INPUT_MIXER.lock().await;
    *master_mixer = Some(mixer);
}

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
    mixer: SyncMixerOutputEnd,
) -> Result<Stream, BuildStreamError> {
    let mixer = Arc::new(std::sync::Mutex::new(mixer));    
    
    device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let mixer = mixer.lock().expect("mixer not available");
            //trace!("Callback wants {:?} ", data.len());
            
            mixdown_sync(mixer.deref(), data);
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
    mixer: SyncMixerInputEnd,
) -> Result<Stream, BuildStreamError> {
    let chcount = config.channels;
    let mixer = Arc::new(std::sync::Mutex::new(mixer));

    device.build_input_stream(
        &config,
        move |data: &[f32], _: &InputCallbackInfo| {
            let m = mixer.lock().expect("failed to open mixer");
            let (consumed, dropped) = transfer_sync(m.deref(), data);
            let dropped = data.len() - consumed;
            
            if dropped > consumed {
                warn!("OVERFLOW - More samples dropped ({}) than consumed ({})", dropped, consumed);
            }
            
            if dropped > 0 {
                trace!("OVERFLOW - Dropped {} Samples", dropped);
            }
            //udp_urge_channel.send(true).unwrap();
        },
        move |err| {},
        None,
    )
}



