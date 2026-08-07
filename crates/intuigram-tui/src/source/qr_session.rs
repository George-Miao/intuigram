/// User action available from the QR-login screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QrLoginAction {
    /// No relevant terminal input is waiting.
    None,

    /// Redraw after a terminal resize.
    Redraw,

    /// Fall back to phone-number authentication.
    PhoneLogin,

    /// Abort Intuigram startup.
    Cancel,
}

/// Temporary alternate-screen session used during Telegram QR login.
pub struct QrLoginUi {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl QrLoginUi {
    /// Enters raw mode and the alternate screen.
    pub fn enter() -> Result<Self> {
        Ok(Self {
            terminal: enter_terminal()?,
        })
    }

    /// Draws the current QR token and its remaining lifetime.
    pub fn draw(&mut self, uri: &str, expires_in: u64) -> Result<()> {
        let qr = qr_login_symbols(uri)?;
        self.terminal
            .draw(|frame| render_qr_login(frame, &qr, expires_in))
            .context(DrawSnafu)?;
        Ok(())
    }

    /// Polls for a QR-screen action without blocking the Telegram receive loop.
    pub fn poll_action(&self, timeout: Duration) -> Result<QrLoginAction> {
        if !event::poll(timeout).context(ReadEventSnafu)? {
            return Ok(QrLoginAction::None);
        }
        match event::read().context(ReadEventSnafu)? {
            Event::Resize(..) => Ok(QrLoginAction::Redraw),
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                match key.code {
                    CrosstermKey::Char('p' | 'P')
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        Ok(QrLoginAction::PhoneLogin)
                    }
                    CrosstermKey::Esc => Ok(QrLoginAction::Cancel),
                    CrosstermKey::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Ok(QrLoginAction::Cancel)
                    }
                    _ => Ok(QrLoginAction::None),
                }
            }
            _ => Ok(QrLoginAction::None),
        }
    }
}

impl Drop for QrLoginUi {
    fn drop(&mut self) {
        restore_terminal(&mut self.terminal);
    }
}
use super::*;
