use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{control::{BufferSize, Port, SampleRate}, mixer::MixerTrackSelector};

#[repr(u8)]
#[derive(Serialize, Deserialize, Debug)]
pub enum ControlError {
    GeneralError,
    StreamIdNotFound,
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