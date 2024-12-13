use carrier_pigeon_common::Message;
use crossterm::event::Event;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use tokio::sync::mpsc;

mod keymap;
mod message_list;

use keymap::{KeyEvent, Keymap, KeymapHandler};
use message_list::{MessageListView, MessageSelector};

pub async fn run(messages: mpsc::UnboundedReceiver<Message>) -> std::io::Result<()> {
    let terminal = ratatui::init();
    let res = run_inner(terminal, messages).await;
    ratatui::restore();
    res
}

#[derive(Debug, Default)]
struct State {
    stopped: bool,
    messages: MessageListView,
    keymaps: Keymaps,
    mode: Mode,
}

#[derive(Debug)]
struct Keymaps {
    message_list: Keymap<Action>,
    normal: Keymap<Action>,
    insert: Keymap<Action>,
    command: Keymap<Action>,
}

impl Default for Keymaps {
    fn default() -> Self {
        let mut message_list = Keymap::default();
        message_list.keys.extend(
            [
                ("q", Action::Quit),
                ("j", Action::SelectMessage(MessageSelector::Relative(1))),
                ("k", Action::SelectMessage(MessageSelector::Relative(-1))),
                ("gg", Action::SelectMessage(MessageSelector::FromStart(0))),
                ("<S-G>", Action::SelectMessage(MessageSelector::FromEnd(0))),
                ("dd", Action::DeleteSelectedMessage),
            ]
            .into_iter()
            .map(|(s, a)| (keymap::parse_key_sequence(s).unwrap(), a)),
        );
        Self {
            message_list,
            normal: Keymap::default(),
            insert: Keymap::default(),
            command: Keymap::default(),
        }
    }
}

impl Keymaps {
    fn active_keymap(&self, mode: Mode) -> &Keymap<Action> {
        match mode {
            Mode::MessageList => &self.message_list,
            Mode::Normal => &self.normal,
            Mode::Insert => &self.insert,
            Mode::Command => &self.command,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Mode {
    /// Main view, with the message list selected
    #[default]
    MessageList,
    /// Normal mode for editing messages
    Normal,
    /// Insert mode for editing messages
    Insert,
    /// Single-line editing mode for entering commands
    Command,
}

#[derive(Debug, Clone)]
enum Action {
    Quit,
    SelectMessage(MessageSelector),
    // TODO: more general
    DeleteSelectedMessage,
}

impl State {
    fn active_keymap(&self) -> &Keymap<Action> {
        self.keymaps.active_keymap(self.mode)
    }

    fn handle_event(&mut self, event: Event) {
        // TODO: resize, mouse, etc
    }

    fn handle_key_event(&mut self, (keys, action): (&[KeyEvent], Option<Action>)) {
        self.insert_keys(keys);
        match action {
            Some(Action::Quit) => self.stopped = true,
            Some(Action::SelectMessage(selector)) => self.messages.select(selector),
            Some(Action::DeleteSelectedMessage) => self.messages.delete_selected(),
            None => {}
        }
    }

    /// Insert keypresses into the active input field, if in insert mode
    fn insert_keys(&mut self, keys: &[KeyEvent]) {
        match self.mode {
            Mode::Insert => todo!(),
            Mode::Command => todo!(),
            Mode::MessageList | Mode::Normal => {}
        }
    }

    fn handle_message(&mut self, message: Message) {
        self.messages.insert(message);
    }
}

impl Widget for &mut State {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.messages.render(area, buffer)
    }
}

macro_rules! handle_event {
    ($state:expr, $event:expr, $handler:ident, $stream_name:literal) => {{
        let state = &mut $state;
        let Some(event) = $event else {
            ::tracing::info!(concat!($stream_name, " stopped, shutting down"));
            state.stopped = true;
            continue;
        };
        state.$handler(event);
    }};
}

macro_rules! select_events {
    ($state:ident; $($name:literal: $stream:expr => $handler:ident),+ $(,)?) => {
        tokio::select! {
            $(e = $stream => handle_event!($state, e, $handler, $name),)*
        }
    }
}

async fn run_inner(
    mut term: ratatui::DefaultTerminal,
    mut messages: mpsc::UnboundedReceiver<Message>,
) -> std::io::Result<()> {
    let mut state = State::default();

    let (key_events, mut term_events) = event_handler();
    let mut keymap = KeymapHandler::new(key_events);
    while !state.stopped {
        term.draw(|frame| frame.render_widget(&mut state, frame.area()))?;
        select_events! {
            state;
            "term events": term_events.recv() => handle_event,
            "key events": keymap.next_cloned(state.active_keymap()) => handle_key_event,
            "message stream": messages.recv() => handle_message,
        };
    }
    Ok(())
}

fn event_handler() -> (
    mpsc::UnboundedReceiver<keymap::KeyEvent>,
    mpsc::UnboundedReceiver<Event>,
) {
    let (key_event_tx, key_event_rx) = mpsc::unbounded_channel();
    let (other_event_tx, other_event_rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || event_handler_inner(key_event_tx, other_event_tx));
    (key_event_rx, other_event_rx)
}

fn event_handler_inner(
    key_event_tx: mpsc::UnboundedSender<keymap::KeyEvent>,
    other_event_tx: mpsc::UnboundedSender<Event>,
) {
    const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

    while !(key_event_tx.is_closed() || other_event_tx.is_closed()) {
        match crossterm::event::poll(TIMEOUT) {
            Ok(true) => match crossterm::event::read() {
                Ok(Event::Key(ev)) => {
                    let _ = key_event_tx.send(ev.into());
                }
                Ok(ev) => {
                    let _ = other_event_tx.send(ev);
                }
                Err(e) => tracing::warn!("failed to read terminal events: {e}"),
            },
            Ok(false) => {}
            Err(e) => tracing::warn!("failed to poll terminal events: {e}"),
        }
    }
}
