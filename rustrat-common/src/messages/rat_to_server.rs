use serde::{Deserialize, Serialize};
use crate::encryption::*;
use crate::error::*;
use crate::messages::{deserialize, serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct CheckIn(PublicKey);

#[derive(Deserialize, Serialize, Clone)]
pub struct EncryptedMessage {
    pub public_key: PublicKey,
    pub data: Encrypted,
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Message {
    CheckIn(PublicKey),
    EncryptedMessage(EncryptedMessage),
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Request {
    NumberOfPendingTasks,
    GetPendingTask,
    Exit,
}

impl EncryptedMessage {
    pub fn to_request(&self, shared_key: SharedKey) -> Result<Request> {
        let plaintext = self.data.decrypt(shared_key)?;
        let request: Request = deserialize(&plaintext)?;

        Ok(request)
    }
}

impl Request {
    pub fn to_encrypted_message<CR: rand::Rng + rand::CryptoRng>(&self, public_key: PublicKey, shared_key: SharedKey, rng: &mut CR) -> Result<EncryptedMessage> {
        let serialized_object = serialize(&self)?;
        let ciphertext = Encrypted::from_byte_array(shared_key, serialized_object, rng)?;

        Ok(EncryptedMessage { 
            public_key: public_key,
            data: ciphertext,
        })
    }
}