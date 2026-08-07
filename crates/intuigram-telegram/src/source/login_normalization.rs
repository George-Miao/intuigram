pub(crate) fn normalize_code_delivery(
    delivery: tl::enums::auth::SentCodeType,
) -> LoginCodeDelivery {
    match delivery {
        tl::enums::auth::SentCodeType::App(delivery) => LoginCodeDelivery::TelegramApp {
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::Sms(delivery) => LoginCodeDelivery::Sms {
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::Call(delivery) => LoginCodeDelivery::PhoneCall {
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::FlashCall(delivery) => LoginCodeDelivery::FlashCall {
            pattern: delivery.pattern,
        },
        tl::enums::auth::SentCodeType::MissedCall(delivery) => LoginCodeDelivery::MissedCall {
            prefix: delivery.prefix,
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::EmailCode(delivery) => LoginCodeDelivery::Email {
            pattern: delivery.email_pattern,
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::SetUpEmailRequired(_) => {
            LoginCodeDelivery::EmailSetupRequired
        }
        tl::enums::auth::SentCodeType::FragmentSms(delivery) => LoginCodeDelivery::Fragment {
            url: delivery.url,
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::FirebaseSms(delivery) => LoginCodeDelivery::FirebaseSms {
            length: delivery.length,
        },
        tl::enums::auth::SentCodeType::SmsWord(delivery) => LoginCodeDelivery::SmsWord {
            beginning: delivery.beginning,
        },
        tl::enums::auth::SentCodeType::SmsPhrase(delivery) => LoginCodeDelivery::SmsPhrase {
            beginning: delivery.beginning,
        },
    }
}

pub(crate) fn input_reply_to(
    reply_to: Option<MessageId>,
    thread_root: Option<MessageId>,
) -> Result<Option<tl::enums::InputReplyTo>> {
    reply_to
        .or(thread_root)
        .map(|message| {
            let reply_to_msg_id =
                i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
                    message_id: message.0,
                })?;
            Ok(tl::types::InputReplyToMessage {
                reply_to_msg_id,
                top_msg_id: thread_root
                    .filter(|root| *root != message)
                    .map(|root| {
                        i32::try_from(root.0)
                            .map_err(|_| Error::InvalidMessageId { message_id: root.0 })
                    })
                    .transpose()?,
                reply_to_peer_id: None,
                quote_text: None,
                quote_entities: None,
                quote_offset: None,
                monoforum_peer_id: None,
                todo_item_id: None,
                poll_option: None,
            }
            .into())
        })
        .transpose()
}

pub(crate) const fn normalize_code_delivery_method(
    delivery: &tl::enums::auth::CodeType,
) -> LoginCodeDeliveryMethod {
    match delivery {
        tl::enums::auth::CodeType::Sms => LoginCodeDeliveryMethod::Sms,
        tl::enums::auth::CodeType::Call => LoginCodeDeliveryMethod::PhoneCall,
        tl::enums::auth::CodeType::FlashCall => LoginCodeDeliveryMethod::FlashCall,
        tl::enums::auth::CodeType::MissedCall => LoginCodeDeliveryMethod::MissedCall,
        tl::enums::auth::CodeType::FragmentSms => LoginCodeDeliveryMethod::Fragment,
    }
}

pub(crate) fn direct_data_centers(options: Vec<tl::enums::DcOption>) -> HashMap<i32, SocketAddr> {
    options
        .into_iter()
        .filter_map(|option| {
            let tl::enums::DcOption::Option(option) = option;
            if option.ipv6 || option.media_only || option.cdn || option.tcpo_only {
                return None;
            }
            let ip = option.ip_address.parse().ok()?;
            let port = u16::try_from(option.port).ok()?;
            Some((option.id, SocketAddr::new(ip, port)))
        })
        .collect()
}

pub(crate) fn ensure_production_environment(test_mode: bool) -> Result<()> {
    if test_mode {
        TestDataCenterSnafu.fail()
    } else {
        Ok(())
    }
}

pub(crate) fn rpc_migration_dc(error: &InvocationError, prefix: &str) -> Option<i32> {
    match error {
        InvocationError::Rpc { message, .. } => message.strip_prefix(prefix)?.parse().ok(),
        _ => None,
    }
}

pub(crate) fn login_error_action(error: &InvocationError) -> LoginErrorAction {
    match error {
        InvocationError::Rpc { message, .. }
            if message == "AUTH_RESTART" || message.starts_with("AUTH_RESTART_") =>
        {
            LoginErrorAction::Restart
        }
        InvocationError::Rpc { message, .. } if message == "SESSION_PASSWORD_NEEDED" => {
            LoginErrorAction::RequestPassword
        }
        _ => LoginErrorAction::Propagate,
    }
}

pub(crate) fn qr_login_uri(token: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token);
    format!("tg://login?token={encoded}")
}
use super::*;
