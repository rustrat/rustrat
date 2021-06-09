pub mod polling;

// TODO different traits to mark various characteristics?
// for example whether it can be reconfigured runtime and able to save/restore
// also: sleepable

pub trait Strategy {
    fn run(self);
}
