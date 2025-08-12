pub mod client;
mod packet;
pub mod server;

pub enum ConnectionControlError {
    GeneralError,
}

type BufferSize = usize;
type SampleRate = usize;
type Port = u16;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use log::{debug, error};
    use tokio::sync::mpsc::{channel, unbounded_channel};

    use crate::{
        audio::{set_global_master_input_mixer, set_global_master_output_mixer},
        control::{client::{TcpClient, TcpClientCommands}, server::TcpServer},
        mixer::{
            default_client_mixer, tests::debug_mixer, MixerTrackSelector,
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
                error!("server crashed: {:?}", e);
                assert!(false);
            }
        });
        
        let (s, r) = unbounded_channel();
        let (s_output, _) = default_client_mixer(2, 1024, 44100);
        set_global_master_input_mixer(s_output).await;
        let client = TcpClient::create(String::from(server_address), r, MixerTrackSelector::Mono(0), dummy_sender);
        
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            s.send(TcpClientCommands::Stop).unwrap();
        });
        
        client.await.unwrap();

        debug!("client exited");

        assert!(true)


        //let (result,) = tokio::join!(client);

        // client has shut down cleanly
    }
}
