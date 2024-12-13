use carrier_pigeon_common::Message;
use crossterm::event::Event;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use tokio::sync::mpsc;

mod keymap;
mod message_list;

use keymap::{Keymap, KeymapHandler};
use message_list::MessageListView;

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
                ("j", Action::SelectMessage(MessageSelector::Relative(-1))),
                ("k", Action::SelectMessage(MessageSelector::Relative(1))),
                ("gg", Action::SelectMessage(MessageSelector::FromStart(0))),
                ("G", Action::SelectMessage(MessageSelector::FromEnd(0))),
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

#[derive(Debug, Clone)]
enum MessageSelector {
    FromStart(usize),
    FromEnd(usize),
    Relative(isize),
}

impl State {
    fn active_keymap(&self) -> &Keymap<Action> {
        self.keymaps.active_keymap(self.mode)
    }

    fn handle_event(&mut self, _event: Event) {
        // TODO: resize, mouse, etc
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

async fn run_inner(
    mut term: ratatui::DefaultTerminal,
    mut messages: mpsc::UnboundedReceiver<Message>,
) -> std::io::Result<()> {
    use futures::future::{select, Either};
    use std::pin::pin;

    let mut state = State::default();

    let (key_events, mut term_events) = event_handler();
    let mut keymap = KeymapHandler::new(key_events);
    while !state.stopped {
        term.draw(|frame| frame.render_widget(&mut state, frame.area()))?;
        let event = match select(
            select(
                pin!(term_events.recv()),
                pin!(keymap.next(&state.active_keymap())),
            ),
            pin!(messages.recv()),
        )
        .await
        {
            Either::Left((Either::Left((e, _)), _)) => Either::Left(Either::Left(e)),
            Either::Left((Either::Right((e, _)), _)) => Either::Left(Either::Right(e)),
            Either::Right((e, _)) => Either::Right(e),
        };
        match event {
            Either::Left(Either::Left(Some(event))) => state.handle_event(event),
            Either::Left(Either::Right(Some((_keys, _action)))) => {
                todo!();
            }
            Either::Right(Some(message)) => state.handle_message(message),
            Either::Left(Either::Left(None)) | Either::Left(Either::Right(None)) => {
                tracing::info!("term events stream stopped, shutting down");
                break;
            }
            Either::Right(None) => {
                tracing::info!("message stream stopped, shutting down");
                break;
            }
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
