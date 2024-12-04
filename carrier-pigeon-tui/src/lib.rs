use crossterm::event::{Event, KeyCode, KeyEvent};
use carrier_pigeon_common::{Message, MessageKey, MessageList};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{List, ListItem, ListState, StatefulWidget, Widget},
};
use tokio::sync::mpsc;

#[derive(Debug, Default)]
pub struct MessageListView {
    messages: MessageList,
    cursor: Option<MessageKey>,
    // TODO: filters
}

impl Widget for &MessageListView {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let highlight_symbol = ">";
        let mut selected_idx = 0;
        let items = self
            .messages
            .iter()
            .enumerate()
            .map(|(idx, msg)| {
                if Some(&msg.key) == self.cursor.as_ref() {
                    selected_idx = idx;
                }
                ListItem::new(format!("{msg:?}"))
            })
            .collect::<Vec<_>>();
        let mut state = ListState::default().with_selected(Some(selected_idx));
        StatefulWidget::render(
            List::new(items).highlight_symbol(highlight_symbol),
            area,
            buffer,
            &mut state,
        );
    }
}

pub async fn run(messages: mpsc::UnboundedReceiver<Message>) -> std::io::Result<()> {
    let terminal = ratatui::init();
    let res = run_inner(terminal, messages).await;
    ratatui::restore();
    res
}

async fn run_inner(
    mut term: ratatui::DefaultTerminal,
    mut messages: mpsc::UnboundedReceiver<Message>,
) -> std::io::Result<()> {
    use futures::{future::Either, stream::StreamExt};

    let mut message_list_view = MessageListView::default();

    let mut term_events = crossterm::event::EventStream::new();
    loop {
        term.draw(|frame| frame.render_widget(&message_list_view, frame.area()))?;
        match futures::future::select(term_events.next(), std::pin::pin!(messages.recv())).await {
            Either::Left((Some(event), _)) => match event {
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Char('q'),
                    ..
                })) => break,
                Ok(event) => tracing::debug!("{event:?}"),
                Err(err) => tracing::warn!("{err}"),
            },
            Either::Right((Some(message), _)) => {
                message_list_view.cursor = Some(message.key.clone());
                message_list_view.messages.insert(message);
            }
            Either::Left((None, _)) => tracing::info!("term events stream stopped, shutting down"),
            Either::Right((None, _)) => tracing::info!("message stream stopped, shutting down"),
        }
    }
    Ok(())
}
