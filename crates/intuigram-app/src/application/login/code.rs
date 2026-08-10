use super::super::*;
use super::prompt::sign_in_with_password;

pub(in crate::application) async fn sign_in_with_delivered_code(
    client: &mut Client,
    mut token: LoginCodeToken,
) -> Result<AuthorizedUser> {
    let mut ui = LoginUi::enter().context(TerminalSnafu)?;
    let mut code = String::new();
    let mut error = None;
    loop {
        let description = login_code_prompt(&token);
        let input = ui
            .read(
                LoginPrompt {
                    field: LoginField::LoginCode,
                    label: "Login code",
                    description: &description,
                    error: error.as_deref(),
                    secret: false,
                    can_go_back: false,
                },
                &code,
            )
            .context(TerminalSnafu)?;
        code = match input {
            LoginInput::Submit(code) => code,
            LoginInput::Cancel => return LoginCancelledSnafu.fail(),
            LoginInput::Back => continue,
        };
        if code.trim().is_empty() {
            error = Some("Login code must not be empty".to_owned());
            continue;
        }
        if code.eq_ignore_ascii_case("resend") {
            match client.resend_login_code(&token).await {
                Ok(CodeRequest::Sent(next_token)) => {
                    token = next_token;
                    code.clear();
                    error = None;
                    continue;
                }
                Ok(CodeRequest::AlreadyAuthorized(user)) => return Ok(user),
                Err(source) => {
                    error = Some(source.to_string());
                    continue;
                }
            }
        }
        match client.sign_in_with_code(&token, code.clone()).await {
            Ok(CodeSignIn::Authorized(user)) => return Ok(user),
            Ok(CodeSignIn::PasswordRequired(password)) => {
                drop(ui);
                return sign_in_with_password(client, password).await;
            }
            Err(source) if source.is_connection_failure() => {
                return Err(Error::Telegram { source });
            }
            Err(source) => error = Some(source.to_string()),
        }
    }
}

fn login_code_prompt(token: &LoginCodeToken) -> String {
    let mut description = login_code_delivery_message(token.delivery());
    if let Some(next) = token.next_delivery() {
        let next = login_code_delivery_method_name(next);
        let suffix = match token.next_delivery_after() {
            Some(seconds) => format!(" Type ‘resend’ after {seconds} seconds to request {next}."),
            None => format!(" Type ‘resend’ to request {next}."),
        };
        description.push_str(&suffix);
    }
    description
}

pub(in crate::application) fn login_code_delivery_message(delivery: &LoginCodeDelivery) -> String {
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

pub(in crate::application) const fn login_code_delivery_method_name(
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
