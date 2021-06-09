use crate::encryption::*;
use crate::error::*;
use crate::messages::{deserialize, serialize};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct CheckIn(PublicKey);

// TODO encrypt at the outmost layer (so that serialized + encrypted is sent instead of encrypted + serialized)

#[derive(Deserialize, Serialize, Clone)]
pub struct EncryptedMessage {
    pub public_key: PublicKey,
    pub data: Encrypted,
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Message {
    CheckIn(EncryptedMessage),
    EncryptedMessage(EncryptedMessage),
}

#[derive(Deserialize, Serialize, Clone)]
pub enum Request {
    Nop,
    NumberOfPendingTasks,
    GetPendingTask,
    // TODO should output be a request or something else?
    Output { task_id: i64, output: String },
    TaskDone { task_id: i64, result: i32 },
    TaskFailed { task_id: i64 },
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
    pub fn to_encrypted_message<CR: rand::Rng + rand::CryptoRng>(
        &self,
        public_key: PublicKey,
        shared_key: SharedKey,
        rng: &mut CR,
    ) -> Result<EncryptedMessage> {
        let serialized_object = serialize(&self)?;
        let ciphertext = Encrypted::from_byte_array(shared_key, serialized_object, rng)?;

        Ok(EncryptedMessage {
            public_key,
            data: ciphertext,
        })
    }
}
