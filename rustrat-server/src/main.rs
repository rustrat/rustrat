use rustrat_common::encryption;

use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
pub async fn main() {
    // TODO move things in main out to other files
    let db_pool = rustrat_server::persistence::prepare_database_pool("rustrat.db")
        .await
        .unwrap();

    // TODO do not hard code private key file location?
    let private_key_file = ".privatekey";
    let private_key = match OpenOptions::new().read(true).open(private_key_file).await {
        Ok(mut file) => {
            let mut key: encryption::PrivateKey = Default::default();
            let mut contents = vec![];
            file.read_to_end(&mut contents).await.unwrap();

            // TODO flag to panic instead of silently overwriting privatekey file?
            if contents.len() != key.len() {
                let mut rng = rand::thread_rng();
                rand::Rng::fill(&mut rng, &mut key);
                drop(file);

                OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(private_key_file)
                    .await
                    .unwrap()
                    .write_all(&key)
                    .await
                    .unwrap();
            }

            key
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

    let mut core_task = rustrat_server::core::CoreTask::new(private_key, db_pool).await;

    {
        let tx = core_task.tx.clone();
        tokio::spawn(async move {
            rustrat_server::listener::web::run(tx).await;
        });
    }

    core_task.run().await;
}
