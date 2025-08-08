use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(name = "mycli")]
pub struct Cli {
    #[structopt(subcommand)]
    pub command: Commands,
    
    #[clap(flatten)]
    pub global: GlobalOptions
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the application in sender mode
    #[clap(name = "sender")]
    Sender(SenderCommands),

    /// Start the application in receiver mode
    #[clap(name = "receiver")]
    Receiver,

    /// Enumerate audio device hardware
    #[clap(name = "devices")]
    EnumDevices,
}

#[derive(Parser, Debug)]
pub struct SenderCommands {
    pub target: String,
}

#[derive(Parser, Debug)]
pub struct GlobalOptions {

    /// Name of the Audio Host to be used (default: Use system default)
    #[arg(short, long)]
    pub audio_host: Option<String>,

    /// Name of the Audio Device to be used (default: Use system default)
    #[arg(short, long)]
    pub device: Option<String>,

    /// Requested Buffer Size
    #[arg(short, long)]
    pub buffer_size: Option<u32>,

    /// Requested Sample Rate
    #[arg(short, long)]
    pub samplerate: Option<u32>,
}