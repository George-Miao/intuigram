use super::super::*;

pub(crate) async fn sign_in_with_password(
    client: &mut Client,
    prompt: intuigram_telegram::PasswordPrompt,
) -> Result<AuthorizedUser> {
    let description = prompt.hint.map_or_else(
        || "Telegram two-factor authentication is enabled.".to_owned(),
        |hint| format!("Telegram two-factor authentication hint: {hint}"),
    );
    let mut ui = LoginUi::enter().context(TerminalSnafu)?;
    let mut error = None;
    loop {
        let input = ui
            .read(
                LoginPrompt {
                    field: LoginField::Password,
                    label: "2FA password",
                    description: &description,
                    error: error.as_deref(),
                    secret: true,
                    can_go_back: false,
                },
                "",
            )
            .context(TerminalSnafu)?;
        let password = match input {
            LoginInput::Submit(password) => password,
            LoginInput::Cancel => return LoginCancelledSnafu.fail(),
            LoginInput::Back => continue,
        };
        if password.is_empty() {
            error = Some("2FA password must not be empty".to_owned());
            continue;
        }
        match client.sign_in_with_password(password.as_bytes()).await {
            Ok(user) => return Ok(user),
            Err(source) if source.is_connection_failure() => {
                return Err(Error::Telegram { source });
            }
            Err(source) => error = Some(source.to_string()),
        }
    }
}

pub(crate) fn prompt_phone_number(initial: &str, initial_error: Option<&str>) -> Result<String> {
    let mut ui = LoginUi::enter().context(TerminalSnafu)?;
    let mut phone = initial.to_owned();
    let mut error = initial_error.map(str::to_owned);
    loop {
        let input = ui
            .read(
                LoginPrompt {
                    field: LoginField::PhoneNumber,
                    label: "Phone number",
                    description: "Include the country calling code, for example +44…",
                    error: error.as_deref(),
                    secret: false,
                    can_go_back: false,
                },
                &phone,
            )
            .context(TerminalSnafu)?;
        phone = match input {
            LoginInput::Submit(phone) => phone,
            LoginInput::Cancel => return LoginCancelledSnafu.fail(),
            LoginInput::Back => continue,
        };
        if phone.trim().is_empty() {
            error = Some("Phone number must not be empty".to_owned());
        } else {
            return Ok(phone.trim().to_owned());
        }
    }
}
