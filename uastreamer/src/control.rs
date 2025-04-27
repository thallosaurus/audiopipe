use core::net;
use std::{
    io::{BufRead, BufReader, BufWriter, Read, Write},
    net::{IpAddr, TcpListener, TcpStream},
    thread::JoinHandle,
};

use serde::{Deserialize, Serialize};

use crate::streamer::Direction;

pub struct TcpCommunication {
    pub direction: Direction,
}

impl TcpCommunication {
    pub fn serve(&self, addr: &str) -> std::io::Result<()> {
        match self.direction {
            Direction::Sender => {
                let mut stream = TcpCommunication::create_new_tcp_stream(addr)?;
                println!("connecting to {}", addr);
                self.sender_loop(&mut stream)?;
            }
            Direction::Receiver => {
                let listener = TcpCommunication::create_new_tcp_listener(addr)?;
                println!("listening to {}", addr);
                self.receiver_loop(listener)?;
            }
        }
        Ok(())
    }
}

impl TcpControlFlow for TcpCommunication {
    fn start_stream(&self) {
        todo!()
    }

    fn connect_to_stream(&self) {
        todo!()
    }
}

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

    fn start_stream(&self);
    fn connect_to_stream(&self);

    fn read_buffer(stream: &mut TcpStream) -> std::io::Result<TcpControlPacket> {
        let mut reader = BufReader::new(stream);

        let mut buf = String::new();
        reader.read_line(&mut buf)?;

        let json = serde_json::from_str(&buf)?;

        dbg!(&json);

        Ok(json)
    }

    fn write_buffer(stream: &TcpStream, packet: TcpControlPacket) -> std::io::Result<()> {
        let mut buf_writer = BufWriter::new(stream);

        let json = serde_json::to_vec(&packet)?;

        // TODO HUH?!?!
        buf_writer.write_all(&json)?;
        buf_writer.write(b"\r\n")?;
        buf_writer.flush()?;
        Ok(())
    }

    fn sender_loop(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        // Start by connecting
        let packet = TcpControlPacket {
            state: TcpControlState::Connect,
        };

        Self::write_buffer(&stream, packet)?;

        let json = Self::read_buffer(stream)?;

        dbg!(&json);

        /*        stream.write_all(json)?;

        // wait for receiver to send back its endpoint address
        let mut s = String::new();
        stream.read_to_string(&mut s)?;

        dbg!(&s);
        //self.connect_to_stream();*/

        match json.state {
            TcpControlState::Endpoint(e) => {
                println!("Connecting to port {}", e);
            }
            _ => todo!(),
        }

        Ok(())
    }

    fn receiver_loop(&self, listener: TcpListener) -> std::io::Result<()> {
        for stream in listener.incoming() {
            println!("Connected");

            let mut stream = stream?;

            let connection_packet = Self::read_buffer(&mut stream)?;

            if connection_packet.state == TcpControlState::Connect {
                //if there is no connection already
                // open new stream
                println!("Opening new Stream");
                let packet = TcpControlPacket {
                    state: TcpControlState::Endpoint(12345),
                };
                dbg!(&packet);

                // send back endpoint
                Self::write_buffer(&stream, packet)?;

                break;
            } else {
                // refuse
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
pub enum TcpControlState {
    Connect,
    Endpoint(u16),
    Disconnect,
    Error,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TcpControlPacket {
    state: TcpControlState,
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use threadpool::ThreadPool;

    use super::*;

    #[test]
    fn test_transfer() {
        let pool = ThreadPool::new(2);

        pool.execute(|| {
            let server = TcpCommunication {
                direction: Direction::Receiver,
            };
            server.serve("127.0.0.1:1234").unwrap();
        });

        // Wait a second for server to be started
        thread::sleep(Duration::from_secs(1));

        pool.execute(|| {
            let client = TcpCommunication {
                direction: Direction::Sender,
            };
            client.serve("127.0.0.1:1234").unwrap();
        });

        pool.join();
    }
}
