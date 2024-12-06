use std::collections::BTreeMap;

use carrier_pigeon_common::{Message, MessageBody, MessageKey, RichText};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Text},
    widgets::{List, ListItem, ListState, StatefulWidget, Widget},
};

#[derive(Debug)]
pub struct MessageListView {
    messages: BTreeMap<MessageKey, Message>,
    cursor: Option<MessageKey>,
    list_state: ListState,
    list_items: List<'static>,
    /// Marks whether the `list_state` and `list_items` are out-of-sync
    dirty: bool,
    // TODO: filters
}

impl Default for MessageListView {
    fn default() -> Self {
        Self {
            messages: Default::default(),
            cursor: None,
            list_state: Default::default(),
            list_items: List::default().highlight_symbol("-> "),
            dirty: false,
        }
    }
}

impl MessageListView {
    pub fn select_next(&mut self) {
        use std::ops::Bound;
        self.cursor = match &self.cursor {
            Some(cursor) => self
                .messages
                .range((Bound::Excluded(cursor), Bound::Unbounded))
                .next(),
            None => self.messages.iter().next(),
        }
        .map(|(k, _)| k.clone())
        .or_else(|| self.cursor.clone());
        self.list_state.select_next();
    }

    pub fn select_prev(&mut self) {
        self.cursor = match &self.cursor {
            Some(cursor) => self.messages.range(..cursor).next_back(),
            None => self.messages.iter().next_back(),
        }
        .map(|(k, _)| k.clone())
        .or_else(|| self.cursor.clone());
        self.list_state.select_previous();
    }

    pub fn select_first(&mut self) {
        self.cursor = self.messages.keys().next().cloned();
        self.list_state.select_first();
    }

    pub fn select_last(&mut self) {
        self.cursor = self.messages.keys().next_back().cloned();
        self.list_state.select_last();
    }

    pub fn insert(&mut self, message: Message) {
        self.messages.insert(message.key(), message);
        self.dirty = true;
    }

    pub fn delete(&mut self, message: &MessageKey) {
        // update the cursor if the message to be deleted is selected
        if self.cursor.as_ref() == Some(message) {
            use std::ops::Bound;
            self.cursor = self
                .messages
                // first try to move the cursor forwards
                .range((Bound::Excluded(message), Bound::Unbounded))
                .next()
                // but if the cursor is already at the end, try moving backwards
                .or_else(|| self.messages.range(..message).next_back())
                .map(|(k, _)| k.clone())
            // if that fails, the deleted message was the only one, so the cursor is now `None`
        }
        self.messages.remove(message);
        self.dirty = true;
    }

    pub fn selected(&self) -> Option<&Message> {
        self.cursor.as_ref().and_then(|key| self.messages.get(key))
    }

    pub fn delete_selected(&mut self) {
        if let Some(selected) = &self.cursor {
            self.delete(&selected.clone());
        }
    }

    fn redraw_list(&mut self) {
        let mut selected_idx = None;
        let items = self
            .messages
            .values()
            .enumerate()
            .map(|(idx, msg)| {
                if Some(&msg.key) == self.cursor.as_ref() {
                    selected_idx = Some(idx);
                }
                ListItem::new(message_to_text(msg))
            })
            .collect::<Vec<_>>();
        self.list_state.select(selected_idx);
        self.list_items = std::mem::take(&mut self.list_items).items(items);
        self.dirty = false;
    }
}

impl Widget for &mut MessageListView {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if self.dirty {
            self.redraw_list();
        }
        StatefulWidget::render(&self.list_items, area, buffer, &mut self.list_state);
    }
}

fn message_to_text(message: &Message) -> Text<'static> {
    // TODO: configuration
    let header = Line::raw(format!(
        "{time} / {room} / {sender} ({sender_id})",
        time = message.key.timestamp,
        // TODO: spaces, threads, replies
        room = message.room.display_name,
        sender = message.sender.display_name,
        sender_id = message.sender.identifier,
    ));
    let body = match &message.body {
        // TODO: wrapping
        MessageBody::Text(RichText(text)) => Line::raw(text.to_string()),
    };
    Text::from(vec![header, body])
}
