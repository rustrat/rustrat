use std::fs;
use std::path::Path;

fn delete_file<P: AsRef<Path>>(path: P) {
    let path = path.as_ref();

    println!("Attempting to delete {}", path.display());
    match fs::remove_file(path) {
        Ok(_) => {
            println!("Delete successful");
        }
        Err(e) => {
            println!("Unable to delete file: {}", e);
        }
    }
}

#[tokio::main]
pub async fn main() {
    delete_file("rustrat-server/dbtemplate.db");
    delete_file("rustrat-server/dbtemplate.db-shm");
    delete_file("rustrat-server/dbtemplate.db-wal");

    rustrat_server::persistence::prepare_database_pool("rustrat-server/dbtemplate.db")
        .await
        .unwrap();
}
