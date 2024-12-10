use std::{cmp, collections::BTreeMap};

use crossterm::event::KeyModifiers;
use tokio::{sync::mpsc, time::Duration};

// in order to resolve a key event, we need to know
// - what mode (keymap) we are in
// - what keys have been pressed already (to handle multi-key sequences)
//
// we also need to handle timeouts on sequences. in particular, in insert mode, any buffered inputs
// need to be passed thru when the timeout expires, so we can't just check the deadline when
// processing a new input

#[derive(Clone, Copy, Debug, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

// manually impl `Ord` since `KeyModifiers` isn't `Ord`
// https://github.com/crossterm-rs/crossterm/pull/951
impl Ord for KeyEvent {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.code
            .cmp(&other.code)
            .then(self.modifiers.bits().cmp(&other.modifiers.bits()))
    }
}

impl PartialOrd for KeyEvent {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for KeyEvent {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == cmp::Ordering::Equal
    }
}

// Our own version of `crossterm::event::KeyCode`
// https://github.com/crossterm-rs/crossterm/pull/951
#[allow(unused)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum KeyCode {
    Char(char),
    Backspace,
    Delete,
    Enter,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    Insert,
    Escape,
    F(u8),
    Unknown,
}

impl From<crossterm::event::KeyCode> for KeyCode {
    fn from(code: crossterm::event::KeyCode) -> Self {
        use crossterm::event::KeyCode as Kc;
        match code {
            Kc::Char(c) => Self::Char(c),
            Kc::Backspace => Self::Backspace,
            Kc::Delete => Self::Delete,
            Kc::Enter => Self::Enter,
            Kc::Left => Self::Left,
            Kc::Right => Self::Right,
            Kc::Up => Self::Up,
            Kc::Down => Self::Down,
            Kc::Home => Self::Home,
            Kc::End => Self::End,
            Kc::PageUp => Self::PageUp,
            Kc::PageDown => Self::PageDown,
            Kc::Tab => Self::Tab,
            Kc::Insert => Self::Insert,
            Kc::Esc => Self::Escape,
            Kc::F(n) => Self::F(n),
            _ => Self::Unknown,
        }
    }
}

pub struct Keymap<A> {
    pub keys: BTreeMap<Vec<KeyEvent>, A>,
    pub timeout: Duration,
}

impl<A: Clone> Keymap<A> {
    pub async fn run(
        &mut self,
        keys_rx: &mut mpsc::UnboundedReceiver<KeyEvent>,
        mut passthru_callback: impl FnMut(&[KeyEvent]),
        mut action_callback: impl FnMut(A),
    ) {
        let mut buffer = Vec::<KeyEvent>::new();
        loop {
            let event = if buffer.is_empty() {
                Ok(keys_rx.recv().await)
            } else {
                tokio::time::timeout(self.timeout, keys_rx.recv()).await
            };
            match event {
                Ok(Some(event)) => {
                    buffer.push(event);
                    let (skipped, action) = (0..buffer.len())
                        .find_map(|i| self.get(&buffer[i..]).map(|action| (i, action)))
                        .unwrap_or((buffer.len(), None));
                    passthru_callback(&buffer[..skipped]);
                    buffer.rotate_left(skipped);
                    buffer.truncate(buffer.len() - skipped);
                    if let Some(action) = action {
                        buffer.clear();
                        action_callback(action);
                    }
                }
                Ok(None) => {
                    tracing::info!("key events stream stopped, shutting down");
                    break;
                }
                Err(_) => {
                    buffer.clear();
                }
            }
        }
    }

    fn entries_with_prefix<'s, 'p>(
        &'s self,
        prefix: &'p [KeyEvent],
    ) -> impl Iterator<Item = (&'s Vec<KeyEvent>, &'s A)> + use<'s, 'p, A> {
        use std::ops::Bound;

        self.keys
            .range::<[_], _>((Bound::Included(prefix), Bound::Unbounded))
            .take_while(move |(k, _)| k.starts_with(prefix))
    }

    /// Finds the action corresponding to the provided key sequence.
    ///
    /// ## Return values
    /// - `Some(Some(action))`: the key sequence is mapped to the action
    /// - `Some(None)`: the key sequence is a prefix to at least one action
    /// - `None`: the key sequence is not a prefix to any action
    fn get(&self, keys: &[KeyEvent]) -> Option<Option<A>> {
        self.entries_with_prefix(keys)
            .next()
            .map(|(k, v)| (k == keys).then_some(v.clone()))
    }
}