use tokio::sync::mpsc::UnboundedSender;

// TODO log time

// TODO hack to get around lifetimes in log::Record, try to send log::Record around instead
// TODO pass additional fields as needed (for example for storage in database)
pub struct Record {
    //level: log::Level,
    //target: String,
    pub msg: String,
}

#[derive(Debug)]
pub struct Logger {
    tx: UnboundedSender<Box<Record>>,
    level: log::Level,
}

impl Logger {
    pub fn new(level: log::Level, tx: UnboundedSender<Box<Record>>) -> Self {
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
