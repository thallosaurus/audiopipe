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
    #[clap(name = "sender")]
    Sender(SenderCommands),

    #[clap(name = "receiver")]
    Receiver,

    #[clap(name = "devices")]
    EnumDevices,
}

#[derive(Parser, Debug)]
pub struct SenderCommands {
    pub target: String,
}

#[derive(Parser, Debug)]
pub struct GlobalOptions {
    #[arg(short, long)]
    pub audio_host: Option<String>,

    #[arg(short, long)]
    pub device: Option<String>,

    #[arg(short, long)]
    pub buffer_size: Option<u32>,

    #[arg(short, long)]
    pub samplerate: Option<u32>,
}