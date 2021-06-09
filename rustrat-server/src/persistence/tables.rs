use std::convert::TryFrom;

use chrono::NaiveDateTime;

//TODO metadata, JSON?
#[derive(sqlx::FromRow)]
pub struct Rat {
    pub rat_id: i64,
    pub public_key: [u8; 32],
    pub first_seen: NaiveDateTime,
    pub last_callback: NaiveDateTime,
    pub alive: bool,
}

pub enum JobType {
    Task,
    Exit,
}

#[derive(sqlx::FromRow)]
pub struct Job {
    pub job_id: i64,
    pub rat_id: i64,
    pub created: NaiveDateTime,
    pub last_update: NaiveDateTime,
    pub started: bool,
    pub done: bool,
    pub job_type: JobType,
    pub payload: Vec<u8>,
}

impl TryFrom<&str> for JobType {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let s = s.to_ascii_lowercase();

        if s == "task" {
            Ok(JobType::Task)
        } else if s == "exit" {
            Ok(JobType::Exit)
        } else {
            Err(())
        }
    }
}

impl From<JobType> for &str {
    fn from(job: JobType) -> Self {
        match job {
            JobType::Task => "task",
            JobType::Exit => "exit",
        }
    }
}
