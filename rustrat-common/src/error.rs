use bincode::Error as BincodeError;
use chacha20poly1305::aead::Error as AeadError;

pub enum Error {
    EncryptionError(AeadError),
    SerializationError(BincodeError),
}

impl From<AeadError> for Error {
    fn from(err: AeadError) -> Error {
        Error::EncryptionError(err)
    }
}

impl From<BincodeError> for Error {
    fn from(err: BincodeError) -> Error {
        Error::SerializationError(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
