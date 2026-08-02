use std::io::{self, Write};

use compio_term::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("failed to initialize the Compio runtime"))]
    Runtime { source: io::Error },

    #[snafu(display("failed to enable terminal raw mode"))]
    EnableRawMode { source: io::Error },

    #[snafu(display("terminal event stream failed"))]
    Event { source: compio_term::EventError },

    #[snafu(display("failed to write the observed event"))]
    WriteEvent { source: io::Error },
}

type Result<T, E = Error> = std::result::Result<T, E>;

struct RawMode;

impl RawMode {
    fn enter() -> Result<Self> {
        crossterm::terminal::enable_raw_mode().context(EnableRawModeSnafu)?;
        Ok(Self)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

fn main() {
    if let Err(error) = run() {
        for (depth, error) in
            std::iter::successors(Some(&error as &dyn std::error::Error), |error| {
                error.source()
            })
            .enumerate()
        {
            if depth == 0 {
                eprintln!("compio-term example: {error}");
            } else {
                eprintln!("  caused by: {error}");
            }
        }
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let runtime = compio::runtime::Runtime::new().context(RuntimeSnafu)?;
    let _raw_mode = RawMode::enter()?;
    runtime.block_on(async {
        let mut events = EventStream::new().context(EventSnafu)?;
        let mut stdout = io::stdout().lock();
        write!(stdout, "Press q or Ctrl+C to stop.\r\n").context(WriteEventSnafu)?;

        while let Some(event) = events.next().await {
            let event = event.context(EventSnafu)?;
            write!(stdout, "{event:?}\r\n").context(WriteEventSnafu)?;
            stdout.flush().context(WriteEventSnafu)?;
            if should_quit(&event) {
                break;
            }
        }
        Ok(())
    })
}

fn should_quit(event: &Event) -> bool {
    let Event::Key(KeyEvent {
        kind: KeyEventKind::Press,
        code: KeyCode::Char(character),
        modifiers,
        ..
    }) = event
    else {
        return false;
    };

    matches!(character, 'q' | 'Q')
        && !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        || matches!(character, 'c' | 'C') && modifiers.contains(KeyModifiers::CONTROL)
}
