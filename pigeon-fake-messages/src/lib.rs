use chrono::Utc;
use pigeon_common::{Message, MessageBody, MessageKey, RichText, Room, User};
use rand::prelude::{Rng, SliceRandom};
use uuid::Uuid;

const ROOM_NAMES: &[&str] = &["general", "random", "memes"];

const USER_NAMES: &[&str] = &["alice", "bob", "charlie", "dana"];

pub async fn message_sender(channel: tokio::sync::mpsc::UnboundedSender<Message>) {
    // set up rooms and users
    let rooms = ROOM_NAMES
        .iter()
        .map(|name| Room {
            display_name: name.to_owned().into(),
            identifier: Uuid::now_v7().to_string().into(),
        })
        .collect::<Vec<_>>();
    let users = USER_NAMES
        .iter()
        .map(|name| User {
            display_name: name.to_owned().into(),
            identifier: format!("@{name}:example.com").into(),
        })
        .collect::<Vec<_>>();

    loop {
        let (message, millis) = generate_message(&rooms, &users);
        if channel.send(message).is_err() {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(millis)).await;
    }
}

fn generate_message(rooms: &[Room], users: &[User]) -> (Message, u64) {
    const MIN_MESSAGE_WORDS: usize = 1;
    const MAX_MESSAGE_WORDS: usize = 15;
    let mut rng = rand::thread_rng();
    let timestamp = Utc::now();
    let identifier = Uuid::now_v7().to_string().into();
    let key = MessageKey {
        timestamp,
        identifier,
    };
    let sender = users.choose(&mut rng).unwrap().clone();
    let room = rooms.choose(&mut rng).unwrap().clone();
    let message_len = rng.gen_range(MIN_MESSAGE_WORDS..=MAX_MESSAGE_WORDS);
    let body = MessageBody::Text(RichText(
        lipsum::lipsum_words_with_rng(&mut rng, message_len).into(),
    ));
    let message = Message {
        key,
        sender,
        room,
        body,
    };
    let millis = rng.gen_range(0..5000);
    (message, millis)
}
