use clap::Parser;
use cpal::{
    Device, StreamConfig,
    traits::{DeviceTrait, HostTrait},
};

use crate::{Direction, args::NewCliArgs, search_device, search_for_host};

#[derive(Clone, Debug)]
pub struct StreamerConfig {
    pub direction: Direction,
    //pub cpal_config: cpal::StreamConfig,
    pub buffer_size: usize,
    //pub channel_count: ChannelCount,
    pub send_stats: bool,
    pub selected_channels: Vec<usize>,
    pub port: u16,
    pub program_args: NewCliArgs,
}

pub fn get_cpal_config(
    direction: Direction,
    audio_host: Option<String>,
    device_name: Option<String>,
) -> anyhow::Result<(Device, StreamConfig)> {
    let host = if let Some(host) = audio_host {
        search_for_host(&host)?
    } else {
        cpal::default_host()
    };

    match direction {
        Direction::Sender => {
            let device = if let Some(device) = device_name {
                host.input_devices()?.find(|x| search_device(x, &device))
            } else {
                host.default_input_device()
            }
            .expect("no input device");

            let default_config = device.default_input_config()?;

            Ok((device, default_config.into()))
        }
        Direction::Receiver => {
            let device = if let Some(device) = device_name {
                host.output_devices()?.find(|x| search_device(x, &device))
            } else {
                host.default_output_device()
            }
            .expect("no output device");

            let default_config = device.default_output_config()?;

            Ok((device, default_config.into()))
        }
    }
}

impl StreamerConfig {
    pub fn from_cli_args(direction: Direction) -> anyhow::Result<Self> {
        let args = NewCliArgs::parse();
        let program_args = args.clone();
        //let (device, config) = get_cpal_config(direction, args.audio_host, args.device).unwrap();

        Ok(Self {
            direction,
            buffer_size: args.buffer_size.unwrap_or(1024),
            send_stats: false,
            selected_channels: args.output_channels,
            port: args.port.unwrap_or(22222),
            program_args,
        })
    }

    pub fn new(
        direction: Direction,
        buffer_size: usize,
        selected_channels: Vec<usize>,
        network_host: Option<String>,

        audio_host: Option<String>,

        device: Option<String>,
        port: Option<u16>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            direction,
            buffer_size,
            send_stats: false,
            selected_channels: selected_channels.clone(),
            port: port.unwrap_or(22222),
            program_args: NewCliArgs {
                //network_host: (),
                device,
                buffer_size: Some(buffer_size),
                port,
                output_channels: selected_channels.clone(),
                network_host,
                audio_host,
            },
        })
    }
}
