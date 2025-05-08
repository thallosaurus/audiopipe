use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};

use crate::{PKG_NAME, VERSION};

#[derive(Parser, Debug, Clone)]
#[command(version, about = format!("{} (v{})", PKG_NAME, VERSION), long_about = None)]
pub struct NewCliArgs {
    /// Name of the Audio Host - REQUIRED if you are the sender
    #[arg(short, long)]
    pub network_host: Option<String>,

    /// Name of the Audio Host - Uses default AudioHost if left empty
    #[arg(short, long)]
    pub audio_host: Option<String>,

    /// Name of the used Audio Device
    #[arg(short, long)]
    pub device: Option<String>,

    /// Buffer Size
    #[arg(short, long)]
    pub buffer_size: Option<usize>,

    /// Port to connect to
    #[arg(short)]
    pub port: Option<u16>,

    /// Selects the tracks to which the app will push data to
    #[clap(short = 'c')]
    pub output_channels: Vec<usize>,

    /// Sets Verbosity
    #[command(flatten)]
    pub verbose: Verbosity
}

impl Default for NewCliArgs {
    fn default() -> Self {
        Self {
            audio_host: Default::default(),
            device: Default::default(),
            buffer_size: Default::default(),
            port: Default::default(),
            output_channels: Default::default(),
            network_host: Default::default(),
            verbose: Verbosity::default(),
        }
    }
}