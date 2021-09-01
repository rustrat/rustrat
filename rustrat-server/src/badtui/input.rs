use crate::badtui::gui::Command;

use futures::StreamExt;

pub struct Input {
    input: String,
    gui_tx: tokio::sync::mpsc::Sender<Command>,
}

impl Input {
    pub async fn handle(
        gui_tx: tokio::sync::mpsc::Sender<Command>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut input = Input {
            input: "".to_string(),
            gui_tx,
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

        tokio::spawn(async move {
            match &cmd_parts[0].to_lowercase() as &str {
                "quit" | "exit" => {
                    log::info!("Shutting down server");
                    gui_tx.send(Command::Quit).await.unwrap();
                }

                _ => {
                    log::info!("Unrecognized command {}", cmd_parts[0]);
                }
            }
        });

        Ok(())
    }
}
