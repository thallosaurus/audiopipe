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

pub enum ConnectionControlError {
    GeneralError,
}

type BufferSize = usize;
type SampleRate = usize;
type Port = u16;

//type SharedUdpServerHandles = Arc<Mutex<HashMap<uuid::Uuid, UdpServerHandle>>>;

#[deprecated]
enum TcpServerCommands {
    Stop,
}

#[deprecated]
pub struct TcpServer {
    pub _task: JoinHandle<io::Result<()>>,
    channel: mpsc::Sender<TcpServerCommands>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_servers() {
        
    }
}