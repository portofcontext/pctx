use serde_json::{Value, json};
use tokio::io::{BufReader, duplex};

use crate::ipc::{read_msg, write_msg};

/// Write a value and read it back through an in-memory pipe.
#[tokio::test]
async fn round_trip_json_value() {
    let (mut writer, reader) = duplex(4096);
    let mut reader = BufReader::new(reader);

    let sent = json!({"hello": "world", "n": 42, "flag": true});
    write_msg(&mut writer, &sent).await.unwrap();

    let received: Value = read_msg(&mut reader).await.unwrap();
    assert_eq!(sent, received);
}

/// Multiple messages are read in order.
#[tokio::test]
async fn multiple_messages_in_order() {
    let (mut writer, reader) = duplex(4096);
    let mut reader = BufReader::new(reader);

    for i in 0u32..5 {
        write_msg(&mut writer, &json!({"i": i})).await.unwrap();
    }

    for i in 0u32..5 {
        let msg: Value = read_msg(&mut reader).await.unwrap();
        assert_eq!(msg["i"], i);
    }
}

/// Closing the write end produces `UnexpectedEof` on the read side.
#[tokio::test]
async fn eof_returns_unexpected_eof_error() {
    let (writer, reader) = duplex(4096);
    let mut reader = BufReader::new(reader);

    // Drop the writer immediately — no bytes written.
    drop(writer);

    let err = read_msg::<_, Value>(&mut reader).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
}

/// Garbled bytes produce an `InvalidData` error, not a panic.
#[tokio::test]
async fn invalid_json_returns_invalid_data_error() {
    use tokio::io::AsyncWriteExt;

    let (mut writer, reader) = duplex(4096);
    let mut reader = BufReader::new(reader);

    writer.write_all(b"not valid json\n").await.unwrap();

    let err = read_msg::<_, Value>(&mut reader).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}
