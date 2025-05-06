use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct UdpAudioPacket {
    data_len: usize,
    data: Vec<u8>,
}

fn main() {
    let data = vec![0, 1, 2, 3, 4];

    let packet = UdpAudioPacket {
        data_len: data.len(),
        data
    };

    let d = bincode2::serialize(&packet).unwrap();
    println!("{:?}", d);
    
    let reser: UdpAudioPacket = bincode2::deserialize(&d).unwrap();
    println!("{:?}", reser);
}