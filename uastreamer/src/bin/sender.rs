use std::{net::Ipv4Addr, str::FromStr};

use clap::Parser;
use cpal::{
    StreamConfig,
    traits::{DeviceTrait, HostTrait},
};
use uastreamer::{
    args::SenderCliArgs, enumerate, search_device, search_for_host, streamer::{self, StreamComponent, Streamer}, streamer_config::StreamerConfig, tui::tui, DEFAULT_PORT, SENDER_BUFFER_SIZE
};

/// Main entrypoint for the sender
fn main() -> anyhow::Result<()> {
    let args = SenderCliArgs::parse();
    // parse audio system host name
    let host = if let Some(host) = args.audio_host {
        search_for_host(&host)?
    } else {
        cpal::default_host()
    };

    // parse device selector
    let device = if let Some(device) = args.device {
        host.input_devices()?.find(|x| search_device(x, &device))
    } else {
        host.default_input_device()
    }
    .expect("no input device");

    if args.enumerate {
        enumerate(streamer::Direction::Sender, &host).unwrap();
        return Ok(());
    }

    println!(
        "Using device {}",
        device.name().unwrap_or("Unknown Device".to_string())
    );

    let default_config = device.default_input_config()?;

    // parse buffer and channel selector
    let buf_size;
    let config = match args.buffer_size {
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

    dbg!(&config);

    let streamer_config = StreamerConfig {
        direction: streamer::Direction::Sender,
        channel_count: config.channels,
        cpal_config: config,
        buffer_size: buf_size as usize,
        send_network_stats: args.ui,
        send_cpal_stats: args.ui,
        selected_channels: vec![0,1],
        port: args.port.unwrap_or(DEFAULT_PORT),
    };

    if let Some(target_server) = args.target_server {
        let sender = Streamer::construct::<f32>(
            Ipv4Addr::from_str(&target_server).expect("Invalid Host Address"),
            &device,
            streamer_config,
        )
        .unwrap();
        if args.ui {
            tui(
                streamer::Direction::Sender,
                &device,
                sender.net_stats,
                sender.cpal_stats,
            )
            .unwrap();
        } else {
            println!("Sending... Press ctrl-c to stop");
            loop {}
        }
    }

    Ok(())
}
