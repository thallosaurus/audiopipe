use std::sync::mpsc::channel;

use audio_streamer::{enumerate, receiver_tui::run_tui, AudioReceiver};
use cpal::traits::{DeviceTrait, HostTrait};

fn main() -> anyhow::Result<()> {
    //enumerate().unwrap();

    let host = cpal::default_host();
    let device = host.default_output_device().expect("no output device available");
    let config = device.default_output_config()?;

    let (stop_tx, stop_rx) = channel();
    
    let receiver = AudioReceiver::new(&device, config, stop_rx).unwrap();

    run_tui(&device, receiver.udp_rx, receiver.cpal_rx).unwrap();
    stop_tx.send(true);

    Ok(())
}