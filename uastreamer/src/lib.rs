use std::{fs::{create_dir_all, File}, io::BufWriter, time::SystemTime};

use bytemuck::Pod;
use cpal::{traits::*, *};
use hound::WavWriter;
use streamer::Direction;

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

/// Holds all things streamer related
pub mod streamer;

/// Holds all things related to the statistics debug window
pub mod tui;

/// Holds everything needed for command line argument parsing
pub mod args;

/// Holds everything related to the audio buffer splitter
pub mod splitter;

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
            },
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
            },
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

    #[cfg(debug_assertions)]
    create_dir_all("dump/")?;

    #[cfg(debug_assertions)]
    let writer = Ok(Some(hound::WavWriter::create(format!("dump/{}_{}.wav", timestamp.as_secs(), filename), spec)?));

    #[cfg(not(debug_assertions))]
    let writer = Ok(None);

    writer
}

pub fn write_debug<T: cpal::SizedSample + Send + Pod + Default + 'static>(writer: &mut Option<DebugWavWriter>, sample: T) {
    if let Some(writer) = writer {
        let s: f32 = bytemuck::cast(sample);
        writer.write_sample(s).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x_test_transfer() {

    }
}