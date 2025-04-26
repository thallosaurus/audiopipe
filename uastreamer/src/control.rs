use core::net;
use std::{
    io::{Read, Write},
    net::{IpAddr, TcpListener, TcpStream},
    thread::JoinHandle,
};

use serde::{Deserialize, Serialize};

/// Contains methods that implement the tcp control functionality
///
/// First, the sender sends a [TcpControlState::Connect] packet with its own configuration
/// and waits for the next packet which must be of [TcpControlState::Endpoint] type.
///
/// On the receiver side, it waits for a [TcpControlState::Connect], starts
/// the Stream and sends back an [TcpControlState::Endpoint] Packet containing
/// the Port for the started stream.
pub trait TcpControlFlow {
    fn create_new_tcp_listener(addr: &str) -> std::io::Result<TcpListener> {
        TcpListener::bind(addr)
    }

    fn create_new_tcp_stream(addr: &str) -> std::io::Result<TcpStream> {
        TcpStream::connect(addr)
    }

    //fn get_tcp_listener(&self) -> TcpListener;
    //fn get_tcp_stream(&self) -> TcpStream;
    fn start_stream(&self);
    fn connect_to_stream(&self);
    fn sender_loop(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        // Start by connecting
        let json = tcp_packet(TcpControlPacket {
            state: TcpControlState::Connect,
        })?;

        stream.write(&json)?;

        // wait for receiver to send back its endpoint address
        let mut s = String::new();
        stream.read_to_string(&mut s)?;

        dbg!(&s);
        self.connect_to_stream();

        Ok(())
    }
    fn receiver_loop(&self, listener: TcpListener) -> std::io::Result<()> {
        for stream in listener.incoming() {
            let mut s = String::new();

            let mut st = stream?;
            st.read_to_string(&mut s)?;

            let json = json_str(s)?;
            dbg!(&json);

            assert_eq!(json.state, TcpControlState::Connect);

            match json.state {
                TcpControlState::Connect => {
                    let packet = TcpControlPacket {
                        state: TcpControlState::Endpoint(12345),
                    };

                    dbg!(&packet);

                    self.start_stream();

                    let endpoint = tcp_packet(packet)?;

                    st.write(&endpoint)?;
                }
                _ => todo!(),
            }
        }
        Ok(())
    }
}

fn tcp_packet(packet: TcpControlPacket) -> std::io::Result<Vec<u8>> {
    Ok(serde_json::to_vec(&packet)?)
}

fn json_str(packet: String) -> std::io::Result<TcpControlPacket> {
    Ok(serde_json::from_str(&packet.as_str())?)
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum TcpControlState {
    Connect,
    Endpoint(u16),
    Disconnect,
}

#[derive(Serialize, Deserialize, Debug)]
struct TcpControlPacket {
    state: TcpControlState,
}
