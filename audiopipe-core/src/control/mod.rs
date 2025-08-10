use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    str::FromStr,
    sync::Arc,
};

use cpal::StreamConfig;
use log::{debug, error, info, trace};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{
        Mutex,
        mpsc::{self},
    },
    task::JoinHandle,
};
use uuid::Uuid;

mod packet;
pub mod server;
pub mod client;

type ConnectionControlResult<T> = Result<T, ()>;

pub enum ConnectionControlError {
    GeneralError,
}

use crate::{
    audio::GLOBAL_MASTER_OUTPUT_MIXER,
    control::packet::{ControlError, ControlRequest, ControlResponse},
    mixer::{MixerTrackSelector, MixerTrait}, streamer::receiver::UdpServerHandle,

};

type BufferSize = usize;
type SampleRate = usize;
type Port = u16;
type ChannelCount = usize;

/// This enum states the type of the tcp control packet.
/// It gets used when the two instances exchange data
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[deprecated]
pub enum ConnectionControlState {
    /// SampleRate, BufferSize, ChannelCount
    ConnectRequest(u32, u32, usize),
    ConnectResponse(String, Port, ChannelCount),
    Disconnect,
    Error(String),
}

/// This is the data that gets sent between two instances
#[derive(Serialize, Deserialize, Debug)]
#[deprecated]
pub struct ConnectionControl {
    pub state: ConnectionControlState,
}

type SharedUdpServerHandles = Arc<Mutex<HashMap<uuid::Uuid, UdpServerHandle>>>;

#[deprecated]
enum TcpServerCommands {
    Stop,
}

#[deprecated]
pub struct TcpServer {
    pub _task: JoinHandle<io::Result<()>>,
    channel: mpsc::Sender<TcpServerCommands>,
}

