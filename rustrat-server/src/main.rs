use std::convert::TryInto;

use once_cell::sync::OnceCell;
use rustrat_common::encryption;
use rustrat_server::badtui;

use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time;

static LOGGER_INSTANCE: OnceCell<rustrat_server::log::Logger> = OnceCell::new();

#[tokio::main]
pub async fn main() {
    let (logger_tx, mut logger_rx) = tokio::sync::mpsc::unbounded_channel();
    let logger = rustrat_server::log::Logger::new(log::Level::Info, logger_tx);
    LOGGER_INSTANCE.set(logger).unwrap();
    log::set_logger(LOGGER_INSTANCE.get().unwrap())
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .unwrap();

    // TODO move things in main out to other files
    let db_pool = rustrat_server::persistence::prepare_database_pool("rustrat.db")
        .await
        .unwrap();

    // TODO do not hard code private key file location?
    let private_key_file = ".privatekey";
    let private_key: encryption::PrivateKey = match OpenOptions::new()
        .read(true)
        .open(private_key_file)
        .await
    {
        Ok(mut file) => {
            let mut contents = vec![];
            file.read_to_end(&mut contents).await.unwrap();

            match contents.try_into() {
                Ok(key) => key,
                // TODO panic or not?
                Err(_) => panic!("Private key file appears corrupted, could not convert file contents to private key."),
            }
        }

        Err(_) => {
            // TODO some way to prevent the server from just overwriting the file?
            let mut key: encryption::PrivateKey = Default::default();
            let mut rng = rand::thread_rng();
            rand::Rng::fill(&mut rng, &mut key);

            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(private_key_file)
                .await
                .unwrap()
                .write_all(&key)
                .await
                .unwrap();

            key
        }
    };

    let public_key =
        x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from(private_key)).to_bytes();
    log::info!(
        "Server starting with public key {}",
        base64::encode_config(public_key, base64::URL_SAFE_NO_PAD)
    );

    let mut core_task = rustrat_server::core::CoreTask::new(private_key, db_pool.clone()).await;

    {
        let tx = core_task.tx.clone();
        tokio::spawn(async move {
            rustrat_server::listener::web::run(tx).await;
        });
    }

    // TODO handle C-c (shutdown), destroy sqlite objects etc
    tokio::spawn(async move {
        core_task.run().await;
    });

    // GUI code
    let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();

    let (gui_tx, gui_rx) = tokio::sync::mpsc::channel(128);

    {
        let db_pool = db_pool.clone();
        std::thread::spawn(move || {
            badtui::gui::Gui::run(gui_rx).unwrap();

            drop(db_pool.writer);
            drop(db_pool.reader);

            finished_tx.send(()).unwrap();
        });
    }

    {
        // Draw task
        let gui_tx = gui_tx.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(time::Duration::from_millis(20));
            interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

            loop {
                interval.tick().await;
                if gui_tx.send(badtui::gui::Command::Draw).await.is_err() {
                    log::debug!("Unable to send draw tick, shutting down task.");
                    break;
                }
            }
        });
    }

    {
        // Input task
        let gui_tx = gui_tx.clone();
        let db_pool = db_pool.clone();

        tokio::spawn(async move {
            if badtui::input::Input::handle(gui_tx, db_pool).await.is_err() {
                log::debug!("Input task returned, shutting down task.");
            }
        });
    }

    {
        // Task to get rats/print new output
        let gui_tx = gui_tx.clone();
        let db_pool = db_pool.clone();

        tokio::spawn(async move {
            let watcher = badtui::watcher::Watcher::new(gui_tx, db_pool);
            match watcher.run().await {
                Ok(_) => unreachable!(),
                Err(_) => log::error!("Watcher task returned error, shutting down task."),
            }
        });
    }

    {
        // Task to read logs
        // TODO is this stupid, first sending to a log channel then just straight to the GUI?
        let gui_tx = gui_tx.clone();

        tokio::spawn(async move {
            while let Some(log_msg) = logger_rx.recv().await {
                gui_tx
                    .send(badtui::gui::Command::SendOutput(log_msg.msg))
                    .await
                    .unwrap();
            }
        });
    }

    finished_rx.await.unwrap();
}
