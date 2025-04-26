use serde::{Deserialize, Serialize};

trait TcpControlStream {
    
}

#[derive(Serialize, Deserialize)]
enum TcpControlState {
    Connect,
    Disconnect
}

#[derive(Serialize, Deserialize)]
struct TcpControlPacket {
    sample_size: u32,
}