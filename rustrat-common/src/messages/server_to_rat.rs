use serde::{Deserialize, Serialize};
use crate::encryption::*;
use crate::error::*;
use crate::messages::{deserialize, serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct EncryptedMessage {
    pub data: Encrypted,
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Message {
    EncryptedMessage(EncryptedMessage),
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Response {
    CheckinSuccessful,
    NumberOfPendingTasks(u32),
    NoTasks,
    Task(Task),
    Exit,
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Task {
    WebAssemblyTask {wasm: Vec<u8>, fn_name: String},
}

impl EncryptedMessage {
    pub fn to_response(&self, shared_key: SharedKey) -> Result<Response> {
        let plaintext = self.data.decrypt(shared_key)?;
        let request: Response = deserialize(&plaintext)?;

        Ok(request)
    }
}

impl Response {
    pub fn to_encrypted_message<CR: rand::Rng + rand::CryptoRng>(&self, shared_key: SharedKey, rng: &mut CR) -> Result<EncryptedMessage> {
        let serialized_object = serialize(&self)?;
        let ciphertext = Encrypted::from_byte_array(shared_key, serialized_object, rng)?;

        Ok(EncryptedMessage { 
            data: ciphertext,
        })
    }
}