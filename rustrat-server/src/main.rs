use std::collections::HashMap;

use tokio::sync::mpsc;

#[tokio::main]
pub async fn main() {
    // TODO only test code to check that the program compiles, remove and replace with actual code
    let pool = rustrat_server::persistence::prepare_database_pool("rustrat.db")
        .await
        .unwrap();

    let mut rng = rand::thread_rng();
    let mut private_key: rustrat_common::encryption::PrivateKey = [0; 32];
    rand::Rng::fill(&mut rng, &mut private_key);

    let mut core_task = rustrat_server::core::CoreTask {
        shared_keys: HashMap::new(),
        private_key: private_key,
        db_pool: pool,
    };

    let (tx, mut rx) = mpsc::channel::<rustrat_server::core::messages::Job>(32);

    while let Some(msg) = rx.recv().await {
        core_task.process_job(msg).await;
    }
}
