use std::sync::{Arc, Mutex};

use clap::Parser;
use cpal::{
    ChannelCount, Device, StreamConfig,
    traits::{DeviceTrait, HostTrait},
};

use crate::{
    args::{ReceiverCliArgs, SenderCliArgs}, search_device, search_for_host, Direction, DEFAULT_PORT, SENDER_BUFFER_SIZE
};

#[derive(Clone, Debug)]
pub struct StreamerConfig {
    pub direction: Direction,
    pub cpal_config: cpal::StreamConfig,
    pub buffer_size: usize,
    pub channel_count: ChannelCount,
    pub send_network_stats: bool,
    pub send_cpal_stats: bool,
    pub selected_channels: Vec<usize>,
    pub port: u16,
}

impl StreamerConfig {
    pub fn from_cli_args(direction: Direction) -> anyhow::Result<(Self, Arc<Mutex<Device>>)> {
        
        let (device, streamer_config) = match direction {
            Direction::Sender => {
                let args = SenderCliArgs::parse();
                // parse audio system host name
                let host = if let Some(host) = args.audio_host {
                    search_for_host(&host)?
                } else {
                    cpal::default_host()
                };
                let device = if let Some(device) = args.device {
                    host.input_devices()?.find(|x| search_device(x, &device))
                } else {
                    host.default_input_device()
                }
                .expect("no input device");

                let default_config = device.default_input_config()?;

                // parse buffer and channel selector
                let buf_size;
                let cpal_config = match args.buffer_size {
                    Some(buffer_size) => {
                        buf_size = buffer_size;

                        StreamConfig {
                            channels: default_config.channels(),
                            sample_rate: cpal::SampleRate(44100),
                            buffer_size: cpal::BufferSize::Fixed(buffer_size),
                        }
                    }
                    _ => {
                        buf_size = SENDER_BUFFER_SIZE as u32;
                        device.default_input_config()?.into()
                    }
                };

                let sconfig = StreamerConfig {
                    direction,
                    channel_count: cpal_config.channels,
                    cpal_config: cpal_config,
                    buffer_size: buf_size as usize,
                    send_network_stats: args.ui,
                    send_cpal_stats: args.ui,
                    selected_channels: vec![0, 1],
                    port: args.port.unwrap_or(DEFAULT_PORT),
                };

                (Arc::new(Mutex::new(device)), sconfig)
            }
            Direction::Receiver => {
                let args = ReceiverCliArgs::parse();
                // parse audio system host name
                let host = if let Some(host) = args.audio_host {
                    search_for_host(&host)?
                } else {
                    cpal::default_host()
                };
                let device = if let Some(device) = args.device {
                    host.output_devices()?.find(|x| search_device(x, &device))
                } else {
                    host.default_output_device()
                }
                .expect("no output device");

                let default_config = device.default_output_config()?;

                // parse buffer and channel selector
                let buf_size;
                let cpal_config = match args.buffer_size {
                    Some(buffer_size) => {
                        buf_size = buffer_size;

                        StreamConfig {
                            channels: default_config.channels(),
                            sample_rate: cpal::SampleRate(44100),
                            buffer_size: cpal::BufferSize::Fixed(buffer_size),
                        }
                    }
                    _ => {
                        buf_size = SENDER_BUFFER_SIZE as u32;
                        device.default_output_config()?.into()
                    }
                };

                let sconfig = StreamerConfig {
                    direction,
                    channel_count: cpal_config.channels,
                    cpal_config: cpal_config,
                    buffer_size: buf_size as usize,
                    send_network_stats: args.ui,
                    send_cpal_stats: args.ui,
                    selected_channels: vec![0, 1],
                    port: args.port.unwrap_or(DEFAULT_PORT),
                };

                (Arc::new(Mutex::new(device)), sconfig)
            },
        };
        // parse device selector

        //if args.enumerate {
        //enumerate(streamer::Direction::Sender, &host).unwrap();
        //}

        println!(
            "Using device {}",
            device.lock().unwrap().name().unwrap_or("Unknown Device".to_string())
        );

        Ok((streamer_config, device))
    }
}
