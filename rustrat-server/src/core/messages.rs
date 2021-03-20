use crate::error::*;

use rustrat_common::messages as common_messages;

use tokio::sync::oneshot;

pub type ReplyChannel = oneshot::Sender<Result<Box<Reply>>>;

pub struct Job {
    pub message: Box<Task>,
    pub reply_channel: ReplyChannel,
}

pub enum Task {
    RatToServer(common_messages::rat_to_server::Message),
}

pub enum Reply {
    ServerToRat(common_messages::server_to_rat::Message),
}
