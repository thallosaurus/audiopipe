use std::{
    fmt::Debug,
    fs::{File, create_dir_all},
    io::BufWriter,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender, channel},
    },
    time::SystemTime,
};

use bytemuck::Pod;

use cpal::{traits::*, *};
use hound::WavWriter;

//use config::{StreamerConfig, get_cpal_config};
use log::{debug, error, info};
use ringbuf::{
    HeapCons, HeapProd,
    traits::{Observer, Split},
};

use crate::async_comp::{audio::{select_input_device_config, select_output_device_config, setup_master_output}, mixer::default_mixer, tcp::new_control_server};

/// Default Port if none is specified
pub const DEFAULT_PORT: u16 = 42069;

/// Default Buffer Size if none is specified
pub const SENDER_BUFFER_SIZE: usize = 1024;

/// Re-export of the Cargo Package Name
pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");

/// Re-export of the Cargo Package Version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Re-export of the Cargo Package Description
pub const PKG_DESC: &str = env!("CARGO_PKG_DESCRIPTION");

/// Holds everything needed for command line argument parsing
//pub mod args;

/// Holds everything related to the audio buffer splitter
pub mod splitter;

/// Holds the config struct which gets passed around
//pub mod config;

/// Holds all flows this app offers
//pub mod pooled;

pub mod ualog;

pub mod async_comp;

pub mod cli;

/// Defines the behavior of the stream
///
/// Sender: Captures from an audio input stream and sends it over the network
/// Receiver: Receives from the network and outputs it to a audio output stream
#[derive(Debug, Clone, Copy)]
#[deprecated]
pub enum Direction {
    Sender,
    Receiver,
}

#[deprecated]
/// Enumerate all available devices on the system
pub fn enumerate(direction: Direction, host: &Host) -> anyhow::Result<()> {
    println!("Supported hosts:\n  {:?}", cpal::ALL_HOSTS);
    let available_hosts = cpal::available_hosts();
    println!("Available hosts:\n  {:?}", available_hosts);

    println!("Devices:");
    for device in host.devices()?.into_iter() {
        match direction {
            Direction::Sender => {
                println!(
                    " - {:?} (Input)",
                    device.name().unwrap_or("Unknown Device".to_string())
                );
                if let Ok(config) = device.supported_input_configs() {
                    for supported_config in config.into_iter() {
                        let buf_size = match supported_config.buffer_size() {
                            SupportedBufferSize::Range { min, max } => format!("{}/{}", min, max),
                            SupportedBufferSize::Unknown => format!("Unknown"),
                        };

                        println!(
                            "   - Buffer Min/Max: {}, Channels: {}, Sample Format: {}, Sample Rate: {:?}",
                            buf_size,
                            supported_config.channels(),
                            supported_config.sample_format(),
                            supported_config.max_sample_rate()
                        )
                    }
                } else {
                    println!("   <not supported>");
                }
                println!("");
            }
            Direction::Receiver => {
                println!(
                    " - {:?} (Output)",
                    device.name().unwrap_or("Unknown Device".to_string())
                );

                if let Ok(conf) = device.supported_output_configs() {
                    for supported_config in conf.into_iter() {
                        let buf_size = match supported_config.buffer_size() {
                            SupportedBufferSize::Range { min, max } => format!("{}/{}", min, max),
                            SupportedBufferSize::Unknown => format!("Unknown"),
                        };

                        println!(
                            "   - Buffer Min/Max: {}, Channels: {}, Sample Format: {}, Sample Rate: {:?}",
                            buf_size,
                            supported_config.channels(),
                            supported_config.sample_format(),
                            supported_config.max_sample_rate()
                        )
                    }
                } else {
                    println!("   <not supported>")
                }
                println!("");
            }
        };
    }

    Ok(())
}

/// Searches for the specified Audio [cpal::HostId] encoded as string
pub fn search_for_host(name: &str) -> anyhow::Result<Host> {
    let host_id = cpal::available_hosts()
        .into_iter()
        .find(|id| id.name() == name)
        .expect("error while retriving sound host name");

    Ok(cpal::host_from_id(host_id)?)
}

/// Lambda function to check if the specified [cpal::Device::name] is the specified name
pub fn search_device(x: &Device, name: &str) -> bool {
    x.name().map(|y| y == name).unwrap_or(false)
}

pub type DebugWavWriter = WavWriter<BufWriter<File>>;

/// Creates the debug wav writer
pub fn create_wav_writer(
    filename: String,
    channels: u16,
    sample_rate: u32,
) -> anyhow::Result<Option<DebugWavWriter>> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let now = SystemTime::now();

    let timestamp = now.duration_since(SystemTime::UNIX_EPOCH)?;

    let mut writer: Option<DebugWavWriter> = None;
    #[cfg(debug_assertions)]
    {
        let fname = format!("dump/{}_{}.wav", timestamp.as_secs(), filename);
        info!("Initializing Debug Wav Writer at {}", fname);
        create_dir_all("dump/")?;
        writer = Some(hound::WavWriter::create(fname, spec)?);
    }

    Ok(writer)
}

pub fn write_debug<T: cpal::SizedSample + Send + Pod + Default + 'static>(
    writer: &mut Option<DebugWavWriter>,
    sample: T,
) {
    if let Some(writer) = writer {
        let s: f32 = bytemuck::cast(sample);
        writer.write_sample(s).unwrap();
    }
}



pub async fn init_sender(
    target: String,
    audio_host: Option<String>,
    device_name: Option<String>,
    bsize: u32,
    srate: u32,
) {

    let (input_device, sconfig) = setup_cpal_input(audio_host, device_name, bsize, srate);
    let master_stream = async_comp::audio::setup_master_input(
        input_device,
        &sconfig,
        bsize as usize,
        vec![0, 1],
    )
    .await
    .expect("couldn't build master output");

    master_stream.play().unwrap();

    // TODO Implement reconnection logic here
    // TODO Check buffersize values here
    async_comp::tcp::tcp_client(&target, &sconfig, 2, bsize)
        .await
        .unwrap();
}

pub async fn init_receiver(
    audio_host: Option<String>,
    device_name: Option<String>,
    bsize: u32,
    srate: u32,
    addr: Option<String>,
) {
    let (output_device, sconfig) = setup_cpal_output(audio_host, device_name, bsize, srate);

    let chcount = sconfig.channels;

    let mixer = default_mixer(chcount as usize, bsize as usize);

    let master_stream = setup_master_output(output_device, sconfig, mixer)
        .await
        .expect("couldn't build master output");

    master_stream.play().unwrap();

    new_control_server(
        String::from(addr.unwrap_or("0.0.0.0".to_string())),
        chcount.into(),
    )
    .await
    .unwrap();
    //server.block();
}

fn setup_cpal_output(
    audio_host: Option<String>,
    device_name: Option<String>,
    bsize: u32,
    srate: u32,
) -> (Device, StreamConfig) {
    let audio_host = match audio_host {
        Some(h) => search_for_host(&h).unwrap(),
        None => cpal::default_host(),
    };

    debug!("Searching for device name: {:?}", device_name);

    let output_device = match device_name {
        Some(d) => audio_host
            .output_devices()
            .unwrap()
            .find(|x| search_device(x, &d)),
        None => audio_host.default_output_device(),
    }
    .expect("no output device");

    let config = select_output_device_config(&output_device, bsize, srate, 2);
    let bs = *config.buffer_size();

    let max_bufsize = match bs {
        cpal::SupportedBufferSize::Range { min, max } => max,
        cpal::SupportedBufferSize::Unknown => panic!("Unknown Supported Buffer Size"),
    };

    info!("max_buffer_size: {}", max_bufsize);

    let sconfig: StreamConfig = config.into();
    let chcount = sconfig.channels;

    info!(
        "Using Audio Device {}, Sample Rate: {}, Buffer Size: {:?}, Channel Count: {}",
        output_device
            .name()
            .unwrap_or("Unknown Device Name".to_string()),
        sconfig.sample_rate.0,
        sconfig.buffer_size,
        chcount
    );

    (output_device, sconfig)
}

fn setup_cpal_input(
    audio_host: Option<String>,
    device_name: Option<String>,
    bsize: u32,
    srate: u32,
) -> (Device, StreamConfig) {
    let audio_host = match audio_host {
        Some(h) => search_for_host(&h).unwrap(),
        None => cpal::default_host(),
    };

    let input_device = match device_name {
        Some(d) => audio_host
            .input_devices()
            .unwrap()
            .find(|x| search_device(x, &d)),
        None => audio_host.default_input_device(),
    }
    .expect("no input device");

    let config = select_input_device_config(&input_device, bsize, srate, 2);

    let bs = *config.buffer_size();
    let max_bufsize = match bs {
        cpal::SupportedBufferSize::Range { min, max } => max,
        cpal::SupportedBufferSize::Unknown => panic!("Unknown Supported Buffer Size"),
    };

    let sconfig: StreamConfig = config.into();

    info!(
        "Using Audio Device {}, Sample Rate: {}, Buffer Size: {:?}, Channel Count: {}",
        input_device
            .name()
            .unwrap_or("Unknown Device Name".to_string()),
        sconfig.sample_rate.0,
        sconfig.buffer_size,
        sconfig.channels
    );

    (input_device, sconfig)
}

pub fn enumerate_devices(audio_host: Option<String>, device_name: Option<String>) {
    let host = match audio_host {
        Some(h) => search_for_host(&h).unwrap(),
        None => cpal::default_host(),
    };

    let output_device = match device_name {
        Some(ref d) => host
            .output_devices()
            .unwrap()
            .find(|x| search_device(x, &d)),
        None => host.default_output_device(),
    }
    .expect("no output device");

    let input_device = match device_name {
        Some(ref d) => host.input_devices().unwrap().find(|x| search_device(x, &d)),
        None => host.default_input_device(),
    }
    .expect("no input device");

    println!("Supported Configs for Device {:?}", input_device.name());
    for c in input_device.supported_input_configs().unwrap() {
        println!(
            "- BufferSize: {:?}, Channels: {}, Min Supported Sample Rate: {}, Max Supported Sample Rate: {}",
            c.buffer_size(),
            c.channels(),
            c.min_sample_rate().0,
            c.max_sample_rate().0
        );
    }

    println!("");

    println!("Supported Configs for Device {:?}", output_device.name());
    for c in output_device.supported_output_configs().unwrap() {
        println!(
            "- BufferSize: {:?}, Channels: {}, Min Supported Sample Rate: {}, Max Supported Sample Rate: {}",
            c.buffer_size(),
            c.channels(),
            c.min_sample_rate().0,
            c.max_sample_rate().0
        );
    }

    println!("");
}


#[cfg(test)]
mod tests {

    #[test]
    fn x_test_transfer() {}

    pub fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }
}
