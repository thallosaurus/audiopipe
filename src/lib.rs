use std::{fs::File, io::BufWriter};

use cpal::{traits::*, *};
use hound::WavWriter;

pub const DEFAULT_PORT: u16 = 42069;
pub const SENDER_BUFFER_SIZE: usize = 1024;

pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PKG_DESC: &str = env!("CARGO_PKG_DESCRIPTION");

/// 
pub mod streamer;
pub mod tui;
pub mod args;

pub fn enumerate(host: &Host) -> anyhow::Result<()> {
    println!("Supported hosts:\n  {:?}", cpal::ALL_HOSTS);
    let available_hosts = cpal::available_hosts();
    println!("Available hosts:\n  {:?}", available_hosts);

    println!("Devices:");
    for device in host.devices()?.into_iter() {
        
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

    Ok(())
}

pub fn search_for_host(name: &str) -> anyhow::Result<Host> {
    let host_id = cpal::available_hosts()
        .into_iter()
        .find(|id| id.name() == name)
        .expect("error while retriving Sound Host Name");

    Ok(cpal::host_from_id(host_id)?)
}

pub fn search_device(x: &Device, name: &str) -> bool {
    x.name().map(|y| y == name).unwrap_or(false)
}

pub fn create_wav_writer(
    filename: String,
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
    sample_format: hound::SampleFormat,
) -> anyhow::Result<WavWriter<BufWriter<File>>> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample,
        sample_format,
    };

    Ok(hound::WavWriter::create(filename, spec)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer() {

        
    }
}