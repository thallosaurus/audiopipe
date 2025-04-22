use std::net::Ipv4Addr;

use audio_streamer::sender;

fn main() {
    let _ = sender(Ipv4Addr::new(10,0,0,1));
}