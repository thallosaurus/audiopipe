use std::fmt::Display;

use log::{debug, trace};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use uuid::Uuid;

use crate::{
    control::{server::TcpServerHandlerErrors, BufferSize, Port, SampleRate},
    mixer::MixerTrackSelector,
    streamer::receiver::UdpServerHandleError,
};

#[repr(u8)]
#[derive(Serialize, Deserialize, Debug)]
pub enum ControlError {
    GeneralError,
    StreamIdNotFound,
    AudioStreamError(UdpServerHandleError),
}

impl Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlError::GeneralError => f.write_fmt(format_args!("general error")),
            ControlError::StreamIdNotFound => f.write_fmt(format_args!("stream not found")),
            ControlError::AudioStreamError(udp_server_handle_error) => {
                f.write_fmt(format_args!("{}", udp_server_handle_error))
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ControlRequest {
    OpenStream(MixerTrackSelector),

    /// CloseStream(StreamId)
    CloseStream(Uuid),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ControlResponse {
    Stream(Uuid, Port, BufferSize, SampleRate),
    Ok,

    // TODO erros
    Error(ControlError),
}

#[derive(Debug)]
pub enum PacketError {
    PacketSendError(io::Error),
    SerdeError(serde_json::Error),
    PacketReadError(io::Error),
}

impl Into<TcpServerHandlerErrors> for PacketError {
    fn into(self) -> TcpServerHandlerErrors {
        TcpServerHandlerErrors::HandlerPacketError(self)
    }
}

pub(crate) async fn read_packet<T>(
    stream: &mut TcpStream,
    mut buf: &mut [u8],
) -> Result<T, PacketError>
where
    T: DeserializeOwned + Serialize + std::fmt::Debug,
{
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| PacketError::PacketReadError(e))?;

    let data = &buf[..n];
    trace!("{:?}", data);
    let json = serde_json::from_slice(data).map_err(|e| PacketError::SerdeError(e))?;
    debug!("{:?}", json);
    Ok(json)
}

pub(crate) async fn send_packet<T>(stream: &mut TcpStream, packet: T) -> Result<(), PacketError>
where
    T: Serialize + std::fmt::Debug,
{
    let json = serde_json::to_vec(&packet).map_err(|e| PacketError::SerdeError(e))?;

    Ok(stream
        .write_all(json.as_slice())
        .await
        .map_err(|e| PacketError::PacketSendError(e))?)
}
