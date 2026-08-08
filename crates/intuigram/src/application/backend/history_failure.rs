use super::*;

pub(super) fn history_failure_event(
    chat: ChatId,
    thread_root: Option<MessageId>,
    error: Error,
) -> Result<Option<AdapterEvent>> {
    match error {
        Error::Telegram { source } if source.is_connection_failure() => {
            Err(Error::Telegram { source })
        }
        Error::Telegram { source } => Ok(Some(AdapterEvent::HistoryLoadFailed {
            chat,
            thread_root,
            reason: source.to_string(),
        })),
        error => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_history_request_is_a_nonfatal_failed_history_event() {
        let event = history_failure_event(
            ChatId(-1_001_195_461_650),
            None,
            Error::Telegram {
                source: intuigram_telegram::Error::PeerUnavailable {
                    chat_id: -1_001_195_461_650,
                },
            },
        )
        .expect("a Telegram request rejection should stay inside the application")
        .expect("a failed history load should notify the state owner");

        assert!(matches!(
            event,
            AdapterEvent::HistoryLoadFailed {
                chat: ChatId(-1_001_195_461_650),
                thread_root: None,
                ..
            }
        ));
    }
}
