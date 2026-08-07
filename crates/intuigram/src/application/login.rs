pub(super) fn seconds_until(expires_at: i32, server_time_offset: i32) -> u64 {
    let local_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let local_now = i64::try_from(local_now).unwrap_or(i64::MAX);
    seconds_until_at(expires_at, local_now, server_time_offset)
}

pub(super) fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

pub(super) fn seconds_until_at(expires_at: i32, local_now: i64, server_time_offset: i32) -> u64 {
    let server_now = local_now.saturating_add(i64::from(server_time_offset));
    u64::try_from(i64::from(expires_at).saturating_sub(server_now)).unwrap_or(0)
}

pub(super) async fn sign_in_with_delivered_code(
    client: &mut Client,
    mut token: LoginCodeToken,
) -> Result<AuthorizedUser> {
    loop {
        print_login_code_delivery(&token);
        let code = prompt("Login code (or 'resend')", "login code")?;
        if code.eq_ignore_ascii_case("resend") {
            match client
                .resend_login_code(&token)
                .await
                .context(TelegramSnafu)?
            {
                CodeRequest::Sent(next_token) => {
                    token = next_token;
                    continue;
                }
                CodeRequest::AlreadyAuthorized(user) => return Ok(user),
            }
        }
        return match client
            .sign_in_with_code(token, code)
            .await
            .context(TelegramSnafu)?
        {
            CodeSignIn::Authorized(user) => Ok(user),
            CodeSignIn::PasswordRequired(password) => sign_in_with_password(client, password).await,
        };
    }
}

pub(super) async fn sign_in_with_password(
    client: &mut Client,
    prompt: intuigram_telegram::PasswordPrompt,
) -> Result<AuthorizedUser> {
    if let Some(hint) = prompt.hint {
        println!("2FA password hint: {hint}");
    }
    let password = rpassword::prompt_password("2FA password: ").context(PromptSnafu {
        field: "2FA password",
    })?;
    if password.is_empty() {
        return EmptyPromptSnafu {
            field: "2FA password",
        }
        .fail();
    }
    client
        .sign_in_with_password(password.as_bytes())
        .await
        .context(TelegramSnafu)
}

pub(super) fn print_login_code_delivery(token: &LoginCodeToken) {
    println!("{}", login_code_delivery_message(token.delivery()));
    if let Some(next) = token.next_delivery() {
        let next = login_code_delivery_method_name(next);
        match token.next_delivery_after() {
            Some(seconds) => println!(
                "If it does not arrive, type 'resend' after {seconds} seconds to request {next}."
            ),
            None => println!("If it does not arrive, type 'resend' to request {next}."),
        }
    }
}

pub(super) fn login_code_delivery_message(delivery: &LoginCodeDelivery) -> String {
    match delivery {
        LoginCodeDelivery::TelegramApp { length } => format!(
            "Telegram sent a {length}-digit code to the Telegram app on another logged-in device."
        ),
        LoginCodeDelivery::Sms { length } => {
            format!("Telegram sent a {length}-digit code by SMS.")
        }
        LoginCodeDelivery::PhoneCall { length } => {
            format!("Telegram will deliver a {length}-digit code by phone call.")
        }
        LoginCodeDelivery::FlashCall { pattern } => format!(
            "Telegram will place a call; use the caller number matching {pattern} as the code."
        ),
        LoginCodeDelivery::MissedCall { prefix, length } => format!(
            "Telegram will place a missed call from a number beginning with {prefix}; use its \
             last {length} digits."
        ),
        LoginCodeDelivery::Email { pattern, length } => {
            format!("Telegram sent a {length}-digit code to {pattern}.")
        }
        LoginCodeDelivery::EmailSetupRequired => {
            "Telegram requires a recovery email to be configured in an official client.".to_owned()
        }
        LoginCodeDelivery::Fragment { length, .. } => {
            format!("Telegram provided a Fragment flow for a {length}-digit code.")
        }
        LoginCodeDelivery::FirebaseSms { length } => {
            format!("Telegram sent a {length}-digit code by Firebase SMS.")
        }
        LoginCodeDelivery::SmsWord { beginning } => match beginning {
            Some(beginning) => {
                format!("Telegram sent an SMS containing a word beginning with {beginning}.")
            }
            None => "Telegram sent an SMS containing a login word.".to_owned(),
        },
        LoginCodeDelivery::SmsPhrase { beginning } => match beginning {
            Some(beginning) => {
                format!("Telegram sent an SMS containing a phrase beginning with {beginning}.")
            }
            None => "Telegram sent an SMS containing a login phrase.".to_owned(),
        },
    }
}

pub(super) const fn login_code_delivery_method_name(
    method: LoginCodeDeliveryMethod,
) -> &'static str {
    match method {
        LoginCodeDeliveryMethod::Sms => "SMS delivery",
        LoginCodeDeliveryMethod::PhoneCall => "a phone call",
        LoginCodeDeliveryMethod::FlashCall => "a caller-number code",
        LoginCodeDeliveryMethod::MissedCall => "a missed-call code",
        LoginCodeDeliveryMethod::Fragment => "Fragment delivery",
    }
}

pub(super) async fn request_code_with_migration(
    credentials: &ApplicationCredentials,
    pending: &AccountDatabase,
    mut client: Client,
    mut session: Session,
    phone_number: &str,
) -> Result<(Client, Session, CodeRequest)> {
    loop {
        match client.request_login_code(phone_number.to_owned()).await {
            Ok(request) => return Ok((client, session, request)),
            Err(error) => {
                let Some(dc_id) = error.phone_migration_dc() else {
                    return Err(Error::Telegram { source: error });
                };
                let endpoint = client
                    .data_center_endpoint(dc_id)
                    .context(MissingDataCenterSnafu { dc_id })?;
                let connected = Client::connect_new(dc_id, endpoint, credentials.clone())
                    .await
                    .context(TelegramSnafu)?;
                client = connected.0;
                session = connected.1;
                pending
                    .save_session(store_session(&session))
                    .context(AccountDatabaseSnafu)?;
            }
        }
    }
}
use super::*;
