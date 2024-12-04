use std::{collections::BTreeMap, sync::Arc};

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

#[derive(Clone, Debug)]
pub enum MessageBody {
    Text(RichText),
    // TODO: other message types
}

// TODO: rich text
#[derive(Clone, Debug)]
pub struct RichText(pub Arc<str>);

#[derive(Clone, Debug, Default)]
pub struct MessageList {
    messages: BTreeMap<MessageKey, Message>,
    // TODO: channel for informing consumers of changes?
}

impl MessageList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, message: Message) -> Option<Message> {
        let key = message.key.clone();
        self.messages.insert(key, message)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Message> {
        self.messages.values()
    }
}
