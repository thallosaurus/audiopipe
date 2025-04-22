use audio_streamer::{enumerate, receiver};

fn main() {
    enumerate().unwrap();
    let _ = receiver();
}