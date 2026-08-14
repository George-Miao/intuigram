use super::super::*;

/// Field shown by the stepped first-run login form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginField {
    /// Telegram application ID.
    ApplicationId,
    /// Telegram application hash.
    ApplicationHash,
    /// Account phone number.
    PhoneNumber,
    /// Delivered login code.
    LoginCode,
    /// Telegram two-factor password.
    Password,
}

impl LoginField {
    pub(super) const fn position(self) -> usize {
        match self {
            Self::ApplicationId => 1,
            Self::ApplicationHash => 2,
            Self::PhoneNumber => 3,
            Self::LoginCode => 4,
            Self::Password => 5,
        }
    }
}

/// Presentation and validation state for one login field.
pub struct LoginPrompt<'a> {
    /// Current step.
    pub field: LoginField,

    /// User-facing field label.
    pub label: &'a str,

    /// Context such as code delivery or password hint.
    pub description: &'a str,

    /// Validation or recoverable adapter error shown in place.
    pub error: Option<&'a str>,

    /// Hide the entered value.
    pub secret: bool,

    /// Permit returning to the prior non-secret step.
    pub can_go_back: bool,
}

/// Completed interaction with one login field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoginInput {
    /// Submit the entered value.
    Submit(String),
    /// Return to the previous step.
    Back,
    /// Cancel first-run setup.
    Cancel,
}

/// Temporary alternate-screen session for centered first-run fields.
pub struct LoginUi {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl LoginUi {
    /// Enters raw mode and the alternate screen.
    pub fn enter() -> Result<Self> {
        Ok(Self {
            terminal: enter_terminal()?,
        })
    }

    /// Edits one field until the user submits, goes back, or cancels.
    pub fn read(&mut self, prompt: LoginPrompt<'_>, initial: &str) -> Result<LoginInput> {
        let mut value = initial.to_owned();
        loop {
            self.terminal
                .draw(|frame| super::render::render_login(frame, &prompt, &value))
                .context(DrawSnafu)?;
            match event::read().context(ReadEventSnafu)? {
                Event::Resize(..) => {}
                Event::Paste(text) => value.push_str(&text),
                Event::Key(key)
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                {
                    match key.code {
                        CrosstermKey::Enter => return Ok(LoginInput::Submit(value)),
                        CrosstermKey::Esc => return Ok(LoginInput::Cancel),
                        CrosstermKey::BackTab if prompt.can_go_back => {
                            return Ok(LoginInput::Back);
                        }
                        CrosstermKey::Backspace => {
                            value.pop();
                        }
                        CrosstermKey::Char(character)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            value.push(character);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

impl Drop for LoginUi {
    fn drop(&mut self) {
        restore_terminal(&mut self.terminal);
    }
}
