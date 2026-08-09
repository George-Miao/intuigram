use std::fs;

use refinery::Target;
use rusqlite::{Connection, params};
use tempfile::tempdir;

use crate::{AccountDatabase, OutboxMedia, OutboxPoll, OutboxState, StoreLayout};

#[test]
fn version_fourteen_outbox_migrates_without_losing_records_or_media() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    fs::create_dir_all(layout.data_directory()).expect("data directory should be created");
    let mut connection =
        Connection::open(layout.pending_database()).expect("fixture database should open");
    super::super::super::migrations::migrations::runner()
        .set_target(Target::Version(14))
        .run(&mut connection)
        .expect("released version fourteen schema should install");
    let media = OutboxMedia::new(
        "proof.bin".to_owned(),
        "application/octet-stream".to_owned(),
        vec![1, 3, 3, 7],
    );
    connection
        .execute(
            "INSERT INTO outbox(operation, state, payload, admitted_at) VALUES ('send', 'ready', \
             ?1, 10)",
            [encoded_payload()],
        )
        .expect("version fourteen Outbox row should insert");
    let id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO outbox_media(outbox_id, position, file_name, mime_type, bytes, sha256) \
             VALUES (?1, 0, ?2, ?3, ?4, ?5)",
            params![
                id,
                media.file_name,
                media.mime_type,
                media.bytes,
                media.sha256
            ],
        )
        .expect("version fourteen retained media should insert");
    drop(connection);

    let database = AccountDatabase::begin_login(&layout)
        .expect("version fourteen database should migrate to the current schema");
    let records = database.load_outbox().expect("migrated Outbox should load");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].media, vec![media]);
    assert!(matches!(
        database.claim_outbox(20).expect("claim should complete"),
        OutboxPoll::Claimed(record) if record.id == records[0].id
    ));
    database
        .cancel_outbox(records[0].id)
        .expect("migrated item should accept cancellation");
    assert_eq!(
        database.load_outbox().expect("Outbox should reload")[0].state,
        OutboxState::CancelRequested
    );
    database
        .mark_outbox_outcome_unknown(records[0].id, "migration proof".to_owned())
        .expect("migrated item should accept an explicit unknown outcome");
    assert_eq!(
        database.load_outbox().expect("Outbox should reload")[0].state,
        OutboxState::OutcomeUnknown
    );
}

fn encoded_payload() -> Vec<u8> {
    let mut bytes = b"IOBX\x01".to_vec();
    bytes.extend_from_slice(&7_i64.to_le_bytes());
    bytes.extend_from_slice(&[0, 0, 0]);
    bytes.extend_from_slice(&10_i64.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes
}
