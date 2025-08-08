use std::str::FromStr;

use clap::{Parser, Subcommand};

pub enum ChannelMappingError {
    InvalidStr,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ChannelMapping {
    from: usize,
    to: usize,
}

impl FromStr for ChannelMapping {
    type Err = ChannelMappingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let sp: Vec<&str> = s.split("=").collect();

        let from = sp.get(0);
        let to = sp.get(0);

        match (from, to) {
            (Some(from), Some(to)) => Ok(Self {
                from: usize::from_str(*from).map_err(|e| ChannelMappingError::InvalidStr)?,
                to: usize::from_str(*to).map_err(|e| ChannelMappingError::InvalidStr)?,
            }),
            _ => Err(ChannelMappingError::InvalidStr),
        }
    }
}

#[derive(Parser)]
#[clap(name = "mycli")]
pub struct Cli {

    /// Enum that holds the subcommand
    #[structopt(subcommand)]
    pub command: Commands,
    
    /// Options, that are globally valid
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