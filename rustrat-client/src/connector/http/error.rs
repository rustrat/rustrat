use crate::ffi::error::Error as FfiError;

#[derive(Debug)]
pub enum Error {
    FfiError(FfiError),
    WinApiError(u32),
}

impl From<FfiError> for Error {
    fn from(error: FfiError) -> Self {
        Error::FfiError(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
