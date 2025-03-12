use std::borrow::Cow;
use std::string::FromUtf8Error;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    StringConversionError(#[from] FromUtf8Error),
    #[error("{0}")]
    Std(#[from] std::io::Error),
    #[error("{0}")]
    Custom(Cow<'static, str>),
}
