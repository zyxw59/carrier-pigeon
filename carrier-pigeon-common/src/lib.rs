use std::sync::Arc;

use chrono::{DateTime, Utc};

#[derive(Clone, Debug)]
pub struct User {
    pub display_name: Arc<str>,
    pub identifier: Arc<str>,
    // TODO: identify service type?
    // TODO: do we care about icons? any other display information?
}

#[derive(Clone, Debug)]
pub struct Room {
    pub display_name: Arc<str>,
    pub identifier: Arc<str>,
    // TODO: identify service type?
    // TODO: parent (space)?
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MessageKey {
    pub timestamp: DateTime<Utc>,
    pub identifier: Arc<str>,
    // TODO: identify service type?
}

#[derive(Clone, Debug)]
pub struct Message {
    pub key: MessageKey,
    pub sender: User,
    // TODO: spaces
    pub room: Room,
    // TODO: threads, replies
    pub body: MessageBody,
}

impl Message {
    pub fn key(&self) -> MessageKey {
        self.key.clone()
    }
}

#[derive(Clone, Debug)]
pub enum MessageBody {
    Text(RichText),
    // TODO: other message types
}

// TODO: rich text
#[derive(Clone, Debug)]
pub struct RichText(pub Arc<str>);
