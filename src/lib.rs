use std::{fs::File, io::BufWriter};

use cpal::{traits::*, *};
use hound::WavWriter;

pub const DEFAULT_PORT: u16 = 42069;
pub const SENDER_BUFFER_SIZE: usize = 1024;

pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PKG_DESC: &str = env!("CARGO_PKG_DESCRIPTION");

pub mod receiver;
pub mod sender;

pub fn enum_new(host: &Host) -> anyhow::Result<()> {
    println!("Supported hosts:\n  {:?}", cpal::ALL_HOSTS);
    let available_hosts = cpal::available_hosts();
    println!("Available hosts:\n  {:?}", available_hosts);

    //let default_in = host.default_input_device().map(|e| e.name().unwrap());
    //let default_out = host.default_output_device().map(|e| e.name().unwrap());

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

pub fn enumerate() -> Result<(), anyhow::Error> {
    let available_hosts = cpal::available_hosts();
    for host_id in available_hosts {
        println!("{}", host_id.name());
        let host = cpal::host_from_id(host_id)?;

        let default_in = host.default_input_device().map(|e| e.name().unwrap());
        let default_out = host.default_output_device().map(|e| e.name().unwrap());
        println!("  Default Input Device:\n    {:?}", default_in);
        println!("  Default Output Device:\n    {:?}", default_out);

        let devices = host.devices()?;
        println!("  Devices: ");
        for (device_index, device) in devices.enumerate() {
            println!("  {}. \"{}\"", device_index + 1, device.name()?);

            // Input configs
            if let Ok(conf) = device.default_input_config() {
                println!("    Default input stream config:\n      {:?}", conf);
            }
            let input_configs = match device.supported_input_configs() {
                Ok(f) => f.collect(),
                Err(e) => {
                    println!("    Error getting supported input configs: {:?}", e);
                    Vec::new()
                }
            };
            if !input_configs.is_empty() {
                println!("    All supported input stream configs:");
                for (config_index, config) in input_configs.into_iter().enumerate() {
                    println!(
                        "      {}.{}. {:?}",
                        device_index + 1,
                        config_index + 1,
                        config
                    );
                }
            }

            // Output configs
            if let Ok(conf) = device.default_output_config() {
                println!("    Default output stream config:\n      {:?}", conf);
            }
            let output_configs = match device.supported_output_configs() {
                Ok(f) => f.collect(),
                Err(e) => {
                    println!("    Error getting supported output configs: {:?}", e);
                    Vec::new()
                }
            };
            if !output_configs.is_empty() {
                println!("    All supported output stream configs:");
                for (config_index, config) in output_configs.into_iter().enumerate() {
                    println!(
                        "      {}.{}. {:?}",
                        device_index + 1,
                        config_index + 1,
                        config
                    );
                }
            }
        }
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
