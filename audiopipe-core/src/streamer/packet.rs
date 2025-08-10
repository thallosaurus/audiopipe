use std::time::{SystemTime, SystemTimeError};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[deprecated]
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct TokioUdpAudioPacket {
    sequence: u64,
    timestamp: SystemTime,
    pub channels: Vec<usize>,
    pub data: Vec<u8>,
}

#[repr(C)]
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct AudioPacketHeader {
    pub connection_id: Uuid,
    pub timestamp: SystemTime,
    pub sample_rate: usize,
    pub channels: usize
}

#[repr(C)]
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct AudioPacket {
    pub header: AudioPacketHeader,
    pub payload: Vec<u8>
}