use std::{net::Ipv4Addr, str::FromStr};

use clap::Parser;
use cpal::{
    StreamConfig,
    traits::{DeviceTrait, HostTrait},
};
use uastreamer::{
    args::ReceiverCliArgs, enumerate, search_device, search_for_host, streamer::{self, StreamComponent, Streamer, StreamerConfig}, tui::tui, DEFAULT_PORT, SENDER_BUFFER_SIZE
};

/// Main entrypoint for the receiver
fn main() -> anyhow::Result<()> {
    let args = ReceiverCliArgs::parse();

    // parse audio system host name
    let host = if let Some(host) = args.audio_host {
        search_for_host(&host)?
    } else {
        cpal::default_host()
    };

    if args.enumerate {
        enumerate(streamer::Direction::Receiver, &host).unwrap();
        return Ok(());
    }

    // parse device selector
    let device = if let Some(device) = args.device {
        //search_for_input_device(host, &device)?
        host.output_devices()?.find(|x| search_device(x, &device))
    } else {
        host.default_output_device()
    }
    .expect("no output device");

    let default_config = device.default_output_config()?;

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
            device.default_output_config()?.into()
        }
    };

    dbg!(&config);

    #[cfg(debug_assertions)]
    let wave_output = args.wave_output;

    #[cfg(not(debug_assertions))]
    let wave_output = false;

    dbg!(&args.output_channels);

    let streamer_config = StreamerConfig {
        direction: streamer::Direction::Receiver,
        channel_count: config.channels,
        cpal_config: config,
        buffer_size: buf_size as usize,
        send_network_stats: args.ui,
        send_cpal_stats: args.ui,
        selected_channels: vec![0,1],
        port: args.port.unwrap_or(DEFAULT_PORT),
    };

    let receiver = Streamer::construct::<f32>(
        Ipv4Addr::from_str("0.0.0.0").expect("Invalid Host Address"),
        &device,
        streamer_config,
    )
    .unwrap();
    if args.ui {
        tui(
            streamer::Direction::Receiver,
            &device,
            receiver.net_stats,
            receiver.cpal_stats,
        )
        .unwrap();
    } else {
        println!("Receiving. Press ctrl+c to exit");
        loop {}
    }

    Ok(())
}
