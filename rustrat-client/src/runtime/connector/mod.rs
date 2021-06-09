pub mod http;

use crate::error::*;

use rustrat_common::messages::{rat_to_server, server_to_rat};

pub trait PollingConnector {
    fn send(&mut self, message: &rat_to_server::Message) -> Result<server_to_rat::Message>;
    fn can_send(&self, message: &rat_to_server::Message) -> bool;
    fn size_limit(&self) -> usize;
}
