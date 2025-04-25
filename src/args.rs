use clap::Parser;

use crate::{PKG_NAME, VERSION};

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

    /// Channel Selector
    #[arg(short, long, requires = "device", requires = "audio_host")]
    pub channel: Option<u16>,

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
    pub port: Option<u16>, // Target IP of the server
                           //#[arg(short)]
                           //pub target_server: String,

    pub stereo: bool,
}

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

    /// Channel Selector
    #[arg(short, long, requires = "device", requires = "audio_host")]
    pub channel: Option<u16>,

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
    
    #[arg(short)]
    pub stereo: bool,
}
