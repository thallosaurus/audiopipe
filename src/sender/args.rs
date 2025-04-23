use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about = "Sender", long_about = None)]
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
    pub target_server: String,

    #[arg(short)]
    pub ui: bool
}