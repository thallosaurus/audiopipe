use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The Header of the Audio Packet
#[repr(C)]
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct AudioPacketHeader {

    /// The id of the connection
    pub connection_id: Uuid,

    // the timestamp of the packet
    pub timestamp: SystemTime,

    /// The sample rate of the packet
    pub sample_rate: usize,

    /// How many channels this packet holds
    pub channels: usize
}

/// The Audiopacket itself
#[repr(C)]
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct AudioPacket {
    pub header: AudioPacketHeader,
    pub payload: Vec<u8>
}