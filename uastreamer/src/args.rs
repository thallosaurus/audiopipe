use clap::Parser;

use crate::{PKG_NAME, VERSION};

#[derive(Parser, Debug, Clone)]
#[command(version, about = format!("{} receiver (v{})", PKG_NAME, VERSION), long_about = None)]
pub struct NewCliArgs {
    /// Name of the Audio Host
    #[arg(short, long)]
    pub network_host: Option<String>,

    /// Name of the Audio Host
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
    #[clap(short)]
    pub output_channels: Vec<usize>,
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
        }
    }
}

#[deprecated]
#[derive(Parser, Debug)]
#[command(version, about = format!("{} receiver (v{})", PKG_NAME, VERSION), long_about = None)]
pub struct ReceiverCliArgs {
    /// Name of the Audio Host
    #[arg(short, long)]
    pub audio_host: Option<String>,

    /// Name of the used Audio Device
    #[arg(short, long)]
    pub device: Option<String>,

    /// Buffer Size
    #[arg(short, long)]
    pub buffer_size: Option<u32>,

    /// Dump Audio Config
    #[arg(short)]
    pub enumerate: bool,

    /// Show Debug TUI
    #[arg(short)]
    pub ui: bool,

    /// Dump received audio to wav file
    #[cfg(debug_assertions)]
    #[arg(short)]
    pub wave_output: bool,

    /// Port to connect to
    #[arg(short)]
    pub port: Option<u16>,

    /// Selects the tracks to which the app will push data to
    #[clap(short)]
    pub output_channels: Vec<usize>,
}

#[deprecated]
#[derive(Parser, Debug)]
#[command(version, about = format!("{} sender (v{})", PKG_NAME, VERSION), long_about = None)]
pub struct SenderCliArgs {
    /// Name of the Audio Host
    #[arg(short, long)]
    pub audio_host: Option<String>,

    /// Name of the used Audio Device
    #[arg(short, long)]
    pub device: Option<String>,

    /// Buffer Size
    #[arg(short, long)]
    pub buffer_size: Option<u32>,

    /// Dump Audio Config
    #[arg(short)]
    pub enumerate: bool,

    /// Target IP of the server
    #[arg(short)]
    pub target_server: Option<String>,

    /// Show Debug TUI
    #[arg(short)]
    pub ui: bool,

    /// Port to listen on
    #[arg(short)]
    pub port: Option<u16>,

    /// Dump audio from buffer to wav file
    #[cfg(debug_assertions)]
    #[arg(short)]
    pub wave_output: bool,

    /// Selects the tracks from which the app will pull samples from
    #[clap(short)]
    pub input_channels: Vec<usize>,
}
