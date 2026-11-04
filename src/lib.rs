#![forbid(unsafe_code)]

pub mod aiff;
pub mod arrangement;
pub mod codec;
pub mod dsp;
pub mod edit;
pub mod engine;
pub mod export;
pub mod flac;
pub mod flp;
pub mod history;
pub mod integrity;
pub mod midi;
pub mod model;
pub mod notes;
pub mod preferences;
pub mod recording;
pub mod recovery;
pub mod routing;
pub mod spectral;
pub mod stems;
pub mod storage;
pub mod stream;
pub mod stretch;
pub mod wav;

use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum Error {
    Invalid(&'static str),
    Limit(&'static str),
    Unsupported(&'static str),
    Io(std::io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(s) | Self::Limit(s) | Self::Unsupported(s) => f.write_str(s),
            Self::Io(e) => Display::fmt(e, f),
        }
    }
}

impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
