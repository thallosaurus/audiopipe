use std::{
    collections::HashMap,
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use tokio::{
    io::{self},
    sync::{
        Mutex,
        mpsc::{self},
    },
    task::JoinHandle,
};

mod packet;
pub mod server;
pub mod client;

type ConnectionControlResult<T> = Result<T, ()>;

pub enum ConnectionControlError {
    GeneralError,
}

use crate::streamer::receiver::UdpServerHandle;

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

