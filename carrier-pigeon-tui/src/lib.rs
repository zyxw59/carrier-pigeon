use carrier_pigeon_common::Message;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use tokio::sync::mpsc;

mod keymap;
mod message_list;

use keymap::Keymap;
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

    fn handle_main_event(&mut self, event: Event) {
        // TODO: configuration
        // TODO: sequences
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                ..
            }) => self.stopped = true,
            Event::Key(KeyEvent {
                code: KeyCode::Char('j'),
                ..
            }) => self.messages.select_next(),
            Event::Key(KeyEvent {
                code: KeyCode::Char('k'),
                ..
            }) => self.messages.select_prev(),
            Event::Key(KeyEvent {
                code: KeyCode::Char('G'),
                ..
            }) => self.messages.select_last(),
            Event::Key(KeyEvent {
                code: KeyCode::Char('g'),
                ..
            }) => self.messages.select_first(),
            Event::Key(KeyEvent {
                code: KeyCode::Char('d'),
                ..
            }) => self.messages.delete_selected(),
            _ => tracing::debug!("{event:?}"),
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

async fn run_inner(
    mut term: ratatui::DefaultTerminal,
    mut messages: mpsc::UnboundedReceiver<Message>,
) -> std::io::Result<()> {
    use futures::{future::Either, stream::StreamExt};

    let mut state = State::default();

    let mut term_events = crossterm::event::EventStream::new();
    while !state.stopped {
        term.draw(|frame| frame.render_widget(&mut state, frame.area()))?;
        match futures::future::select(term_events.next(), std::pin::pin!(messages.recv())).await {
            Either::Left((Some(Ok(event)), _)) => state.handle_event(event),
            Either::Left((Some(Err(err)), _)) => {
                tracing::warn!("error reading terminal event: {err}")
            }
            Either::Right((Some(message), _)) => state.handle_message(message),
            Either::Left((None, _)) => {
                tracing::info!("term events stream stopped, shutting down");
                break;
            }
            Either::Right((None, _)) => {
                tracing::info!("message stream stopped, shutting down");
                break;
            }
        }
    }
    Ok(())
}
