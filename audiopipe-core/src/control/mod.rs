

pub mod client;
mod packet;
pub mod server;

pub enum ConnectionControlError {
    GeneralError,
}

type BufferSize = usize;
type SampleRate = usize;
type Port = u16;

//type SharedUdpServerHandles = Arc<Mutex<HashMap<uuid::Uuid, UdpServerHandle>>>;

#[cfg(test)]
mod tests {
    use log::error;

    use crate::{
        audio::{set_global_master_input_mixer, set_global_master_output_mixer},
        control::{client::TcpClient, server::TcpServer},
        mixer::{
            default_client_mixer, tests::debug_mixer,
        },
        streamer::{receiver::tests::dummy_receiver, sender::tests::dummy_sender},
        tests::init,
    };

    /// this test checks, if the connection handshake works properly
    #[tokio::test]
    async fn test_connection_protocol() {
        init();

        let server_address = "127.0.0.1";

        let (_, r_input) = debug_mixer(2, 1024, 44100);
        set_global_master_output_mixer(r_input).await;

        let server = TcpServer::new(String::from(server_address), dummy_receiver);

        // start the server in the background
        tokio::spawn(async move {
            if let Err(e) = server.await {
                error!("server crashed");
                assert!(false);
            }
        });
        //let server = new_control_server(String::from(server_address), dummy_receiver);

        let (s_output, _) = default_client_mixer(2, 1024, 44100);
        set_global_master_input_mixer(s_output).await;
        let client = TcpClient::new(String::from(server_address), dummy_sender);

        let (result,) = tokio::join!(client);

        // client has shut down cleanly
        assert!(result.is_ok());
    }
}
