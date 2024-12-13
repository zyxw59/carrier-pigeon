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

pub fn parse_key_sequence(input: &str) -> Result<Vec<KeyEvent>, nom::error::Error<&str>> {
    use nom::Finish;
    nom::multi::many1(parse_key)(input).finish().map(|(_, k)| k)
}

fn parse_key(input: &str) -> nom::IResult<&str, KeyEvent> {
    use nom::{
        branch::alt,
        bytes::complete::tag,
        character::complete::one_of,
        combinator::map,
        sequence::{delimited, separated_pair},
    };

    let key = alt((KeyCode::parse_char, KeyCode::parse_special));
    let modifiers = nom::multi::fold_many1(
        map(one_of("ACMS"), |c| match c {
            'A' => KeyModifiers::ALT,
            'C' => KeyModifiers::CONTROL,
            'M' => KeyModifiers::META,
            'S' => KeyModifiers::SHIFT,
            _ => unreachable!(),
        }),
        KeyModifiers::empty,
        KeyModifiers::union,
    );

    let bracketed = alt((
        map(
            separated_pair(modifiers, tag("-"), key),
            |(modifiers, code)| KeyEvent { modifiers, code },
        ),
        map(KeyCode::parse_special, KeyEvent::from),
    ));
    alt((
        delimited(tag("<"), bracketed, tag(">")),
        map(KeyCode::parse_char, KeyEvent::from),
    ))(input)
}

#[derive(Clone, Copy, Debug, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl From<crossterm::event::KeyEvent> for KeyEvent {
    fn from(event: crossterm::event::KeyEvent) -> Self {
        Self {
            code: event.code.into(),
            modifiers: event.modifiers,
        }
    }
}

impl From<KeyCode> for KeyEvent {
    fn from(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::empty(),
        }
    }
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

impl KeyCode {
    fn parse_char(input: &str) -> nom::IResult<&str, Self> {
        nom::combinator::map(
            nom::character::complete::satisfy(nom_unicode::is_alphanumeric),
            Self::Char,
        )(input)
    }

    fn parse_special(input: &str) -> nom::IResult<&str, Self> {
        use nom::{
            bytes::complete::tag,
            combinator::{map, value},
        };
        nom::branch::alt((
            value(Self::Backspace, tag("BS")),
            value(Self::Delete, tag("Del")),
            value(Self::Enter, tag("CR")),
            value(Self::Left, tag("Left")),
            value(Self::Right, tag("Right")),
            value(Self::Up, tag("Up")),
            value(Self::Down, tag("Down")),
            value(Self::Home, tag("Home")),
            value(Self::End, tag("End")),
            value(Self::PageUp, tag("PageUp")),
            value(Self::PageDown, tag("PageDown")),
            value(Self::Tab, tag("Tab")),
            value(Self::Insert, tag("Ins")),
            value(Self::Escape, tag("Esc")),
            map(nom::character::complete::u8, Self::F),
        ))(input)
    }
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

#[derive(Clone, Debug)]
pub struct Keymap<A> {
    pub keys: BTreeMap<Vec<KeyEvent>, A>,
    pub timeout: Duration,
}

const DEFAULT_KEY_TIMEOUT: Duration = tokio::time::Duration::from_millis(500);

impl<A> Default for Keymap<A> {
    fn default() -> Self {
        Self {
            keys: BTreeMap::new(),
            timeout: DEFAULT_KEY_TIMEOUT,
        }
    }
}

impl<A> Keymap<A> {
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
    fn get(&self, keys: &[KeyEvent]) -> Option<Option<&A>> {
        self.entries_with_prefix(keys)
            .next()
            .map(|(k, v)| (k == keys).then_some(v))
    }
}

pub struct KeymapHandler {
    keys_rx: mpsc::UnboundedReceiver<KeyEvent>,
    buffer: Vec<KeyEvent>,
    buffer_skip: usize,
}

impl KeymapHandler {
    pub fn new(keys_rx: mpsc::UnboundedReceiver<KeyEvent>) -> Self {
        Self {
            keys_rx,
            buffer: Vec::new(),
            buffer_skip: 0,
        }
    }

    pub async fn next<'s, 'k, A>(
        &'s mut self,
        keymap: &'k Keymap<A>,
    ) -> Option<(&'s [KeyEvent], Option<&'k A>)> {
        // clear any used keys from the buffer
        self.buffer.drain(..self.buffer_skip).for_each(drop);
        self.buffer_skip = 0;
        let event = if self.buffer.is_empty() {
            Ok(self.keys_rx.recv().await)
        } else {
            tokio::time::timeout(keymap.timeout, self.keys_rx.recv()).await
        };
        match event {
            Ok(Some(event)) => {
                // We store what is essentially a rolling window of recent keypresses. with
                // each new keypress, we check that window against our keymap to see if it is
                // a valid prefix to any mapping. If it is, we then check if it is a complete
                // mapping (not just a prefix), and then return the mapped action. If it is
                // not a valid prefix, we drop the least recent keypress, and repeat.
                //
                // In this manner, except for the most recent keypress, the buffer is always a
                // valid prefix of at least one mapping, so its size is limited by the length
                // of the longest mapping.
                self.buffer.push(event);
                let (skipped, action) = (0..self.buffer.len())
                    .find_map(|i| keymap.get(&self.buffer[i..]).map(|action| (i, action)))
                    .unwrap_or((self.buffer.len(), None));
                self.buffer_skip = skipped;
                if action.is_some() {
                    self.buffer_skip = self.buffer.len();
                }
                Some((&self.buffer[..skipped], action))
            }
            Ok(None) => None,
            Err(_timeout) => {
                self.buffer_skip = self.buffer.len();
                Some((&self.buffer, None))
            }
        }
    }

    pub async fn next_cloned<A: Clone>(
        &mut self,
        keymap: &Keymap<A>,
    ) -> Option<(&[KeyEvent], Option<A>)> {
        self.next(keymap)
            .await
            .map(|(keys, action)| (keys, action.cloned()))
    }
}
