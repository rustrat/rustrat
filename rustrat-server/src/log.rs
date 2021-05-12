use tokio::sync::mpsc;

// TODO log time

// TODO hack to get around lifetimes in log::Record, try to send log::Record around instead
// TODO pass additional fields as needed (for example for storage in database)
struct Record {
    //level: log::Level,
    //target: String,
    msg: String,
}

pub struct Logger {
    tx: mpsc::UnboundedSender<Box<Record>>,
    level: log::Level,
}

impl Logger {
    pub fn new(level: log::Level) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<Box<Record>>();

        // TODO store logs (sqlite?)
        tokio::spawn(async move {
            while let Some(entry) = rx.recv().await {
                println!("{}", entry.msg);
            }
        });

        Logger { tx, level }
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        // Are we at debug or trace level?
        if self.level >= log::Level::Debug {
            // Print everything according to level
            metadata.level() <= self.level
        } else {
            // Otherwise don't print sqlx logs if we are not logging debug/trace
            !metadata.target().starts_with("sqlx") && metadata.level() <= self.level
        }
    }

    // TODO not format message here but offload to logger task?
    fn log(&self, record: &log::Record) {
        let entry = Box::new(Record {
            //level: record.level(),
            //target: record.target().to_string(),
            msg: format!(
                "[{}] {} - {}",
                record.level(),
                record.target(),
                record.args()
            ),
        });

        // TODO avoid panicing? Do we want to continue without logging?
        if self.tx.send(entry).is_err() {
            panic!("Unable to log message, panicing!");
        }
    }

    fn flush(&self) {}
}
