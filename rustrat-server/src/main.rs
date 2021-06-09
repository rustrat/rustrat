use std::convert::TryInto;

use rustrat_common::encryption;

use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref LOGGER: rustrat_server::log::Logger =
        rustrat_server::log::Logger::new(log::Level::Info);
    static ref _INIT_LOG: () = log::set_logger(&*LOGGER)
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .unwrap();
}

#[tokio::main]
pub async fn main() {
    lazy_static::initialize(&_INIT_LOG);

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

    {
        let db_pool = db_pool.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            log::info!("Received signal to shut down, shutting down");

            // TODO shut down tasks

            // TODO figure out what is up with sqlite timing out when starting rustrat-server
            drop(db_pool.writer);
            drop(db_pool.reader);

            // TODO don't exit()? Possible if tasks are shut down?
            std::process::exit(0);
        });
    }

    // TODO handle C-c (shutdown), destroy sqlite objects etc
    core_task.run().await;
}
