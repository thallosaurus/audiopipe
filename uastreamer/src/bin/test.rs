use std::net::UdpSocket;

fn main() {
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    println!("{}", socket.local_addr().unwrap().port());
}