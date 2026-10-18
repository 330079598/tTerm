use super::store::{clear, delete, list, list_all, record, replace_all, send_id};
use super::{validate_key, SerialSendMode, HISTORY_LIMIT};
use crate::db::Database;
use serde_json::json;

const KEY: &str = "profile:board";

fn texts(database: &Database, key: &str) -> Vec<String> {
    database
        .read(|connection| list(connection, key))
        .unwrap()
        .into_iter()
        .map(|send| send.text)
        .collect()
}

#[test]
fn sends_come_back_newest_first_and_a_repeat_moves_up() {
    let database = Database::open_in_memory().unwrap();
    database
        .write(|transaction| {
            record(transaction, KEY, SerialSendMode::Text, "AT", 1)?;
            record(transaction, KEY, SerialSendMode::Text, "AT+GMR", 2)?;
            record(transaction, KEY, SerialSendMode::Hex, "AT", 3)?;
            record(transaction, KEY, SerialSendMode::Text, "AT", 4)?;
            record(transaction, "port:COM3", SerialSendMode::Text, "reboot", 5)
        })
        .unwrap();
    assert_eq!(texts(&database, KEY), ["AT", "AT", "AT+GMR"]);
    let sends = database.read(|connection| list(connection, KEY)).unwrap();
    assert_eq!(sends[0].mode, SerialSendMode::Text);
    assert_eq!(sends[0].sent_at, 4);
    assert_eq!(sends[1].mode, SerialSendMode::Hex);
    assert_eq!(texts(&database, "port:COM3"), ["reboot"]);
}

#[test]
fn each_connection_keeps_only_its_newest_sends() {
    let database = Database::open_in_memory().unwrap();
    database
        .write(|transaction| {
            for index in 0..HISTORY_LIMIT + 5 {
                record(
                    transaction,
                    KEY,
                    SerialSendMode::Text,
                    &format!("cmd {index}"),
                    index as i64,
                )?;
            }
            record(transaction, "port:COM3", SerialSendMode::Text, "kept", 0)
        })
        .unwrap();
    let kept = texts(&database, KEY);
    assert_eq!(kept.len(), HISTORY_LIMIT);
    assert_eq!(kept[0], format!("cmd {}", HISTORY_LIMIT + 4));
    assert_eq!(kept[HISTORY_LIMIT - 1], "cmd 5");
    assert_eq!(texts(&database, "port:COM3"), ["kept"]);
}

#[test]
fn sends_can_be_deleted_one_by_one_or_cleared() {
    let database = Database::open_in_memory().unwrap();
    database
        .write(|transaction| {
            record(transaction, KEY, SerialSendMode::Text, "a", 1)?;
            record(transaction, KEY, SerialSendMode::Text, "b", 2)?;
            record(transaction, "port:COM3", SerialSendMode::Text, "c", 3)?;
            delete(transaction, &[send_id(KEY, SerialSendMode::Text, "a")])
        })
        .unwrap();
    assert_eq!(texts(&database, KEY), ["b"]);
    database
        .write(|transaction| clear(transaction, KEY))
        .unwrap();
    assert!(texts(&database, KEY).is_empty());
    assert_eq!(texts(&database, "port:COM3"), ["c"]);
}

#[test]
fn deleting_a_profile_deletes_its_history() {
    let database = Database::open_in_memory().unwrap();
    let profile = serde_json::from_value(json!({
        "id": "board", "name": "board", "connection_type": "serial",
        "host": "", "port": 22, "username": "",
        "auth_method": "password", "private_key_path": null
    }))
    .unwrap();
    database
        .write(|transaction| {
            crate::profiles::storage::upsert_profile(transaction, &profile)?;
            record(transaction, KEY, SerialSendMode::Text, "AT", 1)?;
            record(transaction, "port:board", SerialSendMode::Text, "AT", 1)?;
            crate::profiles::storage::delete_profiles(transaction, &["board".to_string()])
        })
        .unwrap();
    assert!(texts(&database, KEY).is_empty());
    assert_eq!(texts(&database, "port:board"), ["AT"]);
}

#[test]
fn replacing_keeps_the_limit() {
    let database = Database::open_in_memory().unwrap();
    let sends: Vec<_> = (0..HISTORY_LIMIT + 2)
        .map(|index| {
            let text = format!("cmd {index}");
            super::SerialSend {
                id: send_id(KEY, SerialSendMode::Text, &text),
                connection_key: KEY.to_string(),
                mode: SerialSendMode::Text,
                text,
                sent_at: index as i64,
            }
        })
        .collect();
    database
        .write(|transaction| {
            record(transaction, "port:COM3", SerialSendMode::Text, "gone", 1)?;
            replace_all(transaction, &sends)
        })
        .unwrap();
    let all = database.read(list_all).unwrap();
    assert_eq!(all.len(), HISTORY_LIMIT);
    assert!(all.iter().all(|send| send.connection_key == KEY));
}

#[test]
fn ids_are_the_same_everywhere_and_tell_sends_apart() {
    let id = send_id(KEY, SerialSendMode::Text, "AT");
    assert_eq!(id.len(), 32);
    assert_eq!(id, send_id(KEY, SerialSendMode::Text, "AT"));
    assert_ne!(id, send_id(KEY, SerialSendMode::Hex, "AT"));
    assert_ne!(id, send_id("port:COM3", SerialSendMode::Text, "AT"));
}

#[test]
fn only_send_bar_keys_are_taken() {
    assert!(validate_key("profile:abc").is_ok());
    assert!(validate_key("port:/dev/ttyUSB0").is_ok());
    assert!(validate_key("port: ").is_err());
    assert!(validate_key("COM3").is_err());
    assert!(validate_key("").is_err());
}
