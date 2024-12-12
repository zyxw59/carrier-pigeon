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

#[derive(Debug)]
struct State {
    stopped: bool,
    messages: MessageListView,
    main_keys: Keymap<MainEvent>,
    mode: Mode,
}

const DEFAULT_KEY_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_millis(500);

impl Default for State {
    fn default() -> Self {
        Self {
            stopped: false,
            messages: Default::default(),
            main_keys: Keymap {
                keys: [
                    ("q", MainEvent::Quit),
                    ("j", MainEvent::SelectPrev),
                    ("k", MainEvent::SelectNext),
                    ("gg", MainEvent::SelectFirst),
                    ("G", MainEvent::SelectLast),
                    ("dd", MainEvent::DeleteSelected),
                ]
                .into_iter()
                .map(|(s, a)| (keymap::parse_key_sequence(s).unwrap(), a))
                .collect(),
                timeout: DEFAULT_KEY_TIMEOUT,
            },
            mode: Mode::Main,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Mode {
    /// Main view, with the message list selected
    #[default]
    Main,
}

#[derive(Debug, Clone)]
enum MainEvent {
    Quit,
    SelectPrev,
    SelectNext,
    SelectFirst,
    SelectLast,
    DeleteSelected,
}

impl State {
    fn handle_event(&mut self, event: Event) {
        match self.mode {
            Mode::Main => self.handle_main_event(event),
        }
    }

    fn handle_main_event(&mut self, _event: Event) {
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
                pin!(keymap.next(&state.main_keys)),
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
