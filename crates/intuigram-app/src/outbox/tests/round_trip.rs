use super::fixtures::{mutation_commands, scheduled_commands, send_commands};
use crate::outbox::codec::{decode, encode};

#[test]
fn every_send_family_round_trips_exactly() {
    assert_round_trips(send_commands());
}

#[test]
fn every_scheduled_family_round_trips_exactly() {
    assert_round_trips(scheduled_commands());
}

#[test]
fn every_mutation_family_round_trips_exactly() {
    assert_round_trips(mutation_commands());
}

#[test]
fn encoded_media_is_a_position_not_a_path_or_byte_copy() {
    let encoded = encode(&send_commands()[0]).expect("text command should encode");
    let encoded = str::from_utf8(&encoded[5..]).expect("command body should be JSON");

    assert!(encoded.contains("\"position\":0"));
    assert!(!encoded.contains("\"path\""));
    assert!(!encoded.contains("\"bytes\""));
}

fn assert_round_trips(commands: Vec<super::super::model::PreparedCommand>) {
    for command in commands {
        let destination = command.destination();
        let random_id = command.random_id();
        let local_message_id = command.local_message_id();
        let semantic_command = command.command().clone();
        let encoded = encode(&command).expect("prepared command should encode");

        assert_eq!(&encoded[..4], b"ICMD");
        assert_eq!(encoded[4], 1);
        assert_eq!(
            decode(&encoded).expect("prepared command should decode"),
            command
        );
        assert_eq!(command.destination(), destination);
        assert_eq!(command.random_id(), random_id);
        assert_eq!(command.command(), &semantic_command);
        assert_eq!(command.local_message_id(), local_message_id);
    }
}
