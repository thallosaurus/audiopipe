use std::sync::mpsc::channel;

use audio_streamer::{enumerate, receiver::{args::ReceiverCliArgs, tui::run_tui, AudioReceiver}, search_device, search_for_host, SENDER_BUFFER_SIZE};
use clap::Parser;
use cpal::{traits::{DeviceTrait, HostTrait}, StreamConfig};

fn main() -> anyhow::Result<()> {
    //enumerate().unwrap();

    let args = ReceiverCliArgs::parse();
    if args.enumerate {
        enumerate().unwrap();
        return Ok(());
    } else {
        // parse audio system host name
        let host = if let Some(host) = args.audio_host {
            search_for_host(&host)?
        } else {
            cpal::default_host()
        };

        // parse device selector
        let device = if let Some(device) = args.device {
            //search_for_input_device(host, &device)?
            host.output_devices()?.find(|x| search_device(x, &device))
        } else {
            host.default_output_device()
        }
        .expect("no input device");

        // parse buffer and channel selector
        let buf_size;
        let config = match (args.channel, args.buffer_size) {
            (Some(channel), Some(buffer_size)) => {
                buf_size = buffer_size;

                StreamConfig {
                    channels: channel,
                    sample_rate: cpal::SampleRate(44100),
                    buffer_size: cpal::BufferSize::Fixed(buffer_size),
                }
            }
            (_, _) => {
                buf_size = SENDER_BUFFER_SIZE as u32;
                device.default_output_config()?.into()
            },
        };

        dbg!(&config);
        
        let (stop_tx, stop_rx) = channel();
        
        let receiver = AudioReceiver::new(&device, config, stop_rx, buf_size).unwrap();
        if args.ui {
            run_tui(&device, receiver.udp_rx, receiver.cpal_rx).unwrap();
        } else {
            println!("Receiving. Press ctrl+c to exit");
            loop {}
        }
        //stop_tx.send(true);
    }

    Ok(())
}