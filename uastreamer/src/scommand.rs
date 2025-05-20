use std::{net::SocketAddr, str::FromStr};

use clap::{Args, Parser, Subcommand};
use clap_verbosity_flag::Verbosity;

pub enum ChannelMappingError {
    InvalidStr,
}

struct ChannelMapping {
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

#[derive(Debug, Parser)]
#[clap(name = "test-app", version)]
pub struct AppArgs {
    #[clap(flatten)]
    global_opts: GlobalOpts,

    #[clap(subcommand)]
    command: DirectionCommand,
}

#[derive(Debug, Subcommand, Clone)]
pub enum DirectionCommand {
    Sender {
        /// The TCP Address of the Target Server
        #[clap(long, short = 't')]
        target: SocketAddr,

        /// Selects the tracks to which the app will push data to
        #[clap(long, short = 'c')]
        channels: Vec<usize>,
    },
    Receiver {
        /// Selects the tracks to which the app will push data to
        #[clap(long, short = 'c')]
        channels: Vec<usize>,
    },
}

#[derive(Debug, Args)]
struct GlobalOpts {
    /// Name of the Audio Host - Uses default AudioHost if left empty
    #[arg(short, long)]
    pub audio_host: Option<String>,

    /// Name of the used Audio Device
    #[arg(short, long)]
    pub device: Option<String>,

    #[command(flatten)]
    pub verbose: Verbosity,

    #[arg(short = 'b')]
    buffer_size: usize,

    #[arg(long, short = 'p')]
    port: u16,
}

fn main() {
    let args = AppArgs::parse();
}
