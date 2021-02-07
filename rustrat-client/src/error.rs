#[derive(Debug)]
pub enum Error {
    FfiError(crate::ffi::error::Error),
    WasmError(wasm3::error::Error),
    IoError(std::io::Error),
}

impl From<crate::ffi::error::Error> for Error {
    fn from(error: crate::ffi::error::Error) -> Self {
        Error::FfiError(error)
    }
}

impl From<wasm3::error::Error> for Error {
    fn from(error: wasm3::error::Error) -> Self {
        Error::WasmError(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IoError(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
