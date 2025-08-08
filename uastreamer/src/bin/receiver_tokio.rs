use std::io::Read;

use clap::Parser;
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, BufferSize, StreamConfig
};
use log::{debug, info, trace};
use uastreamer::{
    args::NewCliArgs,
    components::{
        cpal::{select_input_device_config, select_output_device_config},
        tokio::{audio::setup_master_output, tcp::tcp_server},
    },
    search_device, search_for_host,
};

#[tokio::main]
async fn main() {
    env_logger::init();
    let cli = NewCliArgs::parse();

    let audio_host = match cli.audio_host {
        Some(h) => search_for_host(&h).unwrap(),
        None => cpal::default_host(),
    };

    let output_device = match cli.device {
        Some(d) => audio_host
            .output_devices()
            .unwrap()
            .find(|x| search_device(x, &d)),
        None => audio_host.default_output_device(),
    }
    .expect("no output device");

    debug!("Supported Configs for Device {:?}", output_device.name());
    for c in output_device.supported_output_configs().unwrap() {
        debug!(
            "BufferSize: {:?}, Channels: {}, Min Supported Sample Rate: {:?}, Max Supported Sample Rate: {:?}",
            c.buffer_size(),
            c.channels(),
            c.min_sample_rate(),
            c.max_sample_rate()
        );
    }

    let bsize = cli.buffer_size.unwrap_or(1024);
    let srate = cli.samplerate.unwrap_or(44100);

    let config = select_output_device_config(&output_device, bsize, srate, 2);
    let bs = *config.buffer_size();

    let max_bufsize = match bs {
        cpal::SupportedBufferSize::Range { min, max } => max,
        cpal::SupportedBufferSize::Unknown => panic!("Unknown Supported Buffer Size"),
    };

    info!("max_buffer_size: {}", max_bufsize);

    let sconfig: StreamConfig = config.into();

    info!(
        "Using Audio Device {}, Sample Rate: {}, Buffer Size: {:?}, Channel Count: {}",
        output_device
            .name()
            .unwrap_or("Unknown Device Name".to_string()),
        sconfig.sample_rate.0,
        sconfig.buffer_size,
        sconfig.channels
    );

    let master_stream = setup_master_output(output_device, sconfig, max_bufsize as usize, vec![0, 1])
        .await
        .expect("couldn't build master output");

    master_stream.play().unwrap();

    tcp_server("0.0.0.0", 2, bsize).await.unwrap();
}

