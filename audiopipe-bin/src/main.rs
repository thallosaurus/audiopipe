use clap::Parser;
use audiopipe_core::{
    enumerate_devices, init_receiver, init_sender, mixer::MixerTrackSelector,
};

use crate::cli::{Cli, Commands};

pub mod cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    env_logger::init();

    let channel_selector = MixerTrackSelector::Stereo(0, 1);

    let bsize = cli.global.buffer_size.unwrap_or(1024);
    let srate = cli.global.samplerate.unwrap_or(44100);
    match cli.command {
        Commands::Sender(sender_commands) => {
            init_sender(
                sender_commands.target,
                cli.global.audio_host,
                cli.global.device,
                bsize as usize,
                srate as usize,
                channel_selector
            )
            .await
        }
        Commands::Receiver(recv_commands) => {
            init_receiver(
                cli.global.audio_host,
                cli.global.device,
                bsize as usize,
                srate as usize,
                recv_commands.addr,
                channel_selector
            )
            .await
        }
        Commands::EnumDevices => {
            enumerate_devices(cli.global.audio_host, cli.global.device);
        }
    }
}
