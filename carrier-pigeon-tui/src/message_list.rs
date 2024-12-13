use std::collections::BTreeMap;

use carrier_pigeon_common::{Message, MessageBody, MessageKey, RichText};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Text},
    widgets::{List, ListItem, ListState, StatefulWidget, Widget},
};

#[derive(Debug, Clone)]
pub enum MessageSelector {
    FromStart(usize),
    FromEnd(usize),
    Relative(isize),
}

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
            list_state: ListState::default().with_selected(Some(0)),
            list_items: List::default().highlight_symbol("-> "),
            dirty: false,
        }
    }
}

impl MessageListView {
    pub fn select(&mut self, selector: MessageSelector) {
        use std::ops::Bound;
        match selector {
            MessageSelector::FromStart(index) => {
                self.cursor = self.messages.keys().nth(index).cloned();
                *self.list_state.selected_mut() = Some(index);
            }
            MessageSelector::FromEnd(index) => {
                self.cursor = self.messages.keys().nth_back(index).cloned();
                *self.list_state.selected_mut() =
                    Some(self.messages.len().saturating_sub(index) - 1);
            }
            MessageSelector::Relative(0) => {}
            MessageSelector::Relative(offset @ 1..) => {
                let lower_bound = self
                    .cursor
                    .as_ref()
                    .map_or(Bound::Unbounded, Bound::Excluded);
                self.cursor = self
                    .messages
                    .range((lower_bound, Bound::Unbounded))
                    .nth(offset as usize - 1)
                    .map(|(k, _)| k.clone())
                    .or_else(|| self.cursor.clone());
                self.list_state.scroll_down_by(offset as u16);
            }
            MessageSelector::Relative(offset @ ..=-1) => {
                let upper_bound = self
                    .cursor
                    .as_ref()
                    .map_or(Bound::Unbounded, Bound::Excluded);
                self.cursor = self
                    .messages
                    .range((Bound::Unbounded, upper_bound))
                    .nth_back(-(offset + 1) as usize)
                    .map(|(k, _)| k.clone())
                    .or_else(|| self.cursor.clone());
                self.list_state.scroll_up_by((-offset) as u16);
            }
        }
    }

    pub fn insert(&mut self, message: Message) {
        if self.cursor.is_none() {
            self.cursor = Some(message.key());
        }
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
        self.list_state.select(Some(selected_idx.unwrap_or(0)));
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
