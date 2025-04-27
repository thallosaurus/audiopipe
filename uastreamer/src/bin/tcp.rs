use std::time::Duration;

use threadpool::ThreadPool;
use uastreamer::{control::{TcpCommunication, TcpControlFlow}, streamer::Direction};

/// Test implementation for the tcp communication
fn main2() -> std::io::Result<()> {
    let tcp = TcpCommunication {
        direction: uastreamer::streamer::Direction::Receiver
    };

    tcp.serve("0.0.0.0:1234")?;

    Ok(())
}

fn main() {
    let pool = ThreadPool::new(2);

    pool.execute(|| {
        let server = TcpCommunication {
            direction: Direction::Receiver
        };
        server.serve("127.0.0.1:1234").unwrap();
    });

    // Wait a second for server to be started
    std::thread::sleep(Duration::from_secs(1));

    pool.execute(|| {
        let client = TcpCommunication {
            direction: Direction::Sender,
        };
        client.serve("127.0.0.1:1234").unwrap();
    });

    pool.join();
}