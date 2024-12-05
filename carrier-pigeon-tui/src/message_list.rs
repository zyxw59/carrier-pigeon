use carrier_pigeon_common::{Message, MessageKey, MessageList};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{List, ListItem, ListState, StatefulWidget, Widget},
};

#[derive(Debug)]
pub struct MessageListView {
    messages: MessageList,
    cursor: Option<MessageKey>,
    list_state: ListState,
    list_items: List<'static>,
    // TODO: filters
}

impl Default for MessageListView {
    fn default() -> Self {
        Self {
            messages: Default::default(),
            cursor: None,
            list_state: Default::default(),
            list_items: List::default().highlight_symbol("->"),
        }
    }
}

impl MessageListView {
    pub fn select_next(&mut self) {
        use std::ops::Bound;
        self.cursor = match &self.cursor {
            Some(cursor) => self
                .messages
                .range((Bound::Unbounded, Bound::Excluded(cursor)))
                .next(),
            None => self.messages.iter().next(),
        }
        .map(Message::key)
        .or_else(|| self.cursor.clone());
        self.list_state.select_next();
    }

    pub fn select_prev(&mut self) {
        self.cursor = match &self.cursor {
            Some(cursor) => self.messages.range(..cursor).next_back(),
            None => self.messages.iter().next_back(),
        }
        .map(Message::key)
        .or_else(|| self.cursor.clone());
        self.list_state.select_previous();
    }

    pub fn select_first(&mut self) {
        self.cursor = self.messages.iter().next().map(Message::key);
        self.list_state.select_first();
    }

    pub fn select_last(&mut self) {
        self.cursor = self.messages.iter().next_back().map(Message::key);
        self.list_state.select_last();
    }

    pub fn insert(&mut self, message: Message) {
        self.messages.insert(message);
        self.redraw_list();
    }

    fn redraw_list(&mut self) {
        let mut selected_idx = None;
        // TODO: find selected index if `self.cursor` points to a deleted message
        let items = self
            .messages
            .iter()
            .enumerate()
            .map(|(idx, msg)| {
                if Some(&msg.key) == self.cursor.as_ref() {
                    selected_idx = Some(idx);
                }
                ListItem::new(format!("{msg:?}"))
            })
            .collect::<Vec<_>>();
        self.list_state = std::mem::take(&mut self.list_state).with_selected(selected_idx);
        self.list_items = std::mem::take(&mut self.list_items).items(items);
    }
}

impl Widget for &mut MessageListView {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        StatefulWidget::render(&self.list_items, area, buffer, &mut self.list_state);
    }
}
