use super::*;

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
    monoforum_peer: Option<tl::enums::InputPeer>,
) -> Result<Option<tl::enums::InputReplyTo>> {
    match (reply_to.or(thread_root), monoforum_peer) {
        (None, None) => Ok(None),
        (None, Some(monoforum_peer_id)) => Ok(Some(
            tl::types::InputReplyToMonoForum { monoforum_peer_id }.into(),
        )),
        (Some(message), monoforum_peer_id) => {
            let reply_to_msg_id =
                i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
                    message_id: message.0,
                })?;
            Ok(Some(
                tl::types::InputReplyToMessage {
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
                    monoforum_peer_id,
                    todo_item_id: None,
                    poll_option: None,
                }
                .into(),
            ))
        }
    }
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

pub(crate) fn direct_data_centers(options: Vec<tl::enums::DcOption>) -> DataCenterEndpoints {
    data_centers(options, |option| !option.media_only && !option.cdn)
}

pub(crate) fn media_data_centers(options: Vec<tl::enums::DcOption>) -> DataCenterEndpoints {
    data_centers(options, |option| option.media_only && !option.cdn)
}

pub(crate) fn cdn_data_centers(options: Vec<tl::enums::DcOption>) -> DataCenterEndpoints {
    data_centers(options, |option| option.cdn)
}

fn data_centers(
    options: Vec<tl::enums::DcOption>,
    accepts: impl Fn(&tl::types::DcOption) -> bool,
) -> DataCenterEndpoints {
    let mut data_centers = HashMap::<i32, Vec<SocketAddr>>::new();
    for option in options {
        let tl::enums::DcOption::Option(option) = option;
        if option.tcpo_only || !accepts(&option) {
            continue;
        }
        let Ok(ip) = option.ip_address.parse::<std::net::IpAddr>() else {
            continue;
        };
        if ip.is_ipv6() != option.ipv6 {
            continue;
        }
        let Ok(port) = u16::try_from(option.port) else {
            continue;
        };
        let endpoint = SocketAddr::new(ip, port);
        let endpoints = data_centers.entry(option.id).or_default();
        if !endpoints.contains(&endpoint) {
            endpoints.push(endpoint);
        }
    }
    data_centers
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

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(id: i32, address: &str, cdn: bool) -> tl::enums::DcOption {
        tl::types::DcOption {
            ipv6: address.contains(':'),
            media_only: false,
            tcpo_only: false,
            cdn,
            r#static: false,
            this_port_only: false,
            id,
            ip_address: address.to_owned(),
            port: 443,
            secret: None,
        }
        .into()
    }

    fn peer() -> tl::enums::InputPeer {
        tl::types::InputPeerUser {
            user_id: 20,
            access_hash: 30,
        }
        .into()
    }

    #[test]
    fn monoforum_sends_address_the_user_dialog_with_or_without_a_reply() {
        let direct = input_reply_to(None, None, Some(peer()))
            .expect("a monoforum peer should be valid")
            .expect("a monoforum send always has reply addressing");
        assert!(matches!(direct, tl::enums::InputReplyTo::MonoForum(_)));

        let reply = input_reply_to(Some(MessageId(7)), None, Some(peer()))
            .expect("a monoforum reply should be valid")
            .expect("a reply should have addressing");
        assert!(matches!(
            reply,
            tl::enums::InputReplyTo::Message(message)
                if message.reply_to_msg_id == 7 && message.monoforum_peer_id.is_some()
        ));
    }

    #[test]
    fn direct_data_centers_keep_ipv4_and_ipv6() {
        let selected = direct_data_centers(vec![
            endpoint(2, "149.154.167.41", false),
            endpoint(2, "2001:67c:4e8:f002::a", false),
        ]);
        let ipv4 = "149.154.167.41:443"
            .parse()
            .expect("IPv4 fixture should parse");
        let ipv6 = "[2001:67c:4e8:f002::a]:443"
            .parse()
            .expect("IPv6 fixture should parse");

        assert_eq!(
            selected.get(&2).map(Vec::as_slice),
            Some([ipv4, ipv6].as_slice())
        );
    }

    #[test]
    fn cdn_data_centers_only_keep_cdn_endpoints() {
        let selected = cdn_data_centers(vec![
            endpoint(4, "149.154.167.51", false),
            endpoint(203, "91.108.56.200", true),
        ]);
        let expected = "91.108.56.200:443".parse().expect("fixture should parse");

        assert_eq!(
            selected.get(&203).map(Vec::as_slice),
            Some([expected].as_slice())
        );
        assert!(!selected.contains_key(&4));
    }
}
