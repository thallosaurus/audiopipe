use std::fmt::Display;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{control::{BufferSize, Port, SampleRate}, mixer::MixerTrackSelector, streamer::receiver::UdpServerHandleError};

#[repr(u8)]
#[derive(Serialize, Deserialize, Debug)]
pub enum ControlError {
    GeneralError,
    StreamIdNotFound,
    AudioStreamError(UdpServerHandleError)
}

impl Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlError::GeneralError => f.write_fmt(format_args!("general error")),
            ControlError::StreamIdNotFound => f.write_fmt(format_args!("stream not found")),
            ControlError::AudioStreamError(udp_server_handle_error) => f.write_fmt(format_args!("{}", udp_server_handle_error)),
        }
    }
}


#[derive(Serialize, Deserialize, Debug)]
pub enum ControlRequest {
    OpenStream(MixerTrackSelector),
    
    /// CloseStream(StreamId)
    CloseStream(Uuid)
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ControlResponse {
    Stream(Uuid, Port, BufferSize, SampleRate),
    Ok,

    // TODO erros
    Error(ControlError)
}