use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use snafu::{OptionExt, ResultExt};

use super::{MissingPipeSnafu, RejectedSnafu, Result, SpawnSnafu, WaitSnafu, WriteSnafu, png};
use crate::{CellSize, Placement};

const PROGRAM: &str = "chafa";

/// Safe argv for a caller-owned asynchronous Chafa process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChafaCommand {
    /// Executable name.
    pub program: &'static str,

    /// Arguments without shell interpolation.
    pub arguments: Vec<String>,
}

impl ChafaCommand {
    /// Builds a symbols-only Chafa request for an encoded image file.
    #[must_use]
    pub fn symbols(path: &Path, size: CellSize) -> Self {
        let mut command = Self::symbols_stdin(size);
        command.arguments.pop();
        command.arguments.push(path.to_string_lossy().into_owned());
        command
    }

    pub(crate) fn symbols_stdin(size: CellSize) -> Self {
        Self {
            program: PROGRAM,
            arguments: vec![
                "--format=symbols".to_owned(),
                "--colors=full".to_owned(),
                format!("--size={}x{}", size.columns, size.rows),
                "--animate=off".to_owned(),
                "--probe=off".to_owned(),
                "--polite=on".to_owned(),
                "--relative=off".to_owned(),
                "--".to_owned(),
                "-".to_owned(),
            ],
        }
    }
}

pub(super) fn render(placement: &Placement) -> Result<Vec<u8>> {
    let request = ChafaCommand::symbols_stdin(placement.size);
    let mut child = Command::new(request.program)
        .args(&request.arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context(SpawnSnafu { program: PROGRAM })?;
    child
        .stdin
        .take()
        .context(MissingPipeSnafu {
            program: PROGRAM,
            pipe: "stdin",
        })?
        .write_all(&png(&placement.image)?)
        .context(WriteSnafu { program: PROGRAM })?;
    let output = child
        .wait_with_output()
        .context(WaitSnafu { program: PROGRAM })?;
    if !output.status.success() {
        return RejectedSnafu {
            program: PROGRAM,
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }
        .fail();
    }
    let mut rendered = format!(
        "\x1b7\x1b[{};{}H",
        placement.y.saturating_add(1),
        placement.x.saturating_add(1)
    )
    .into_bytes();
    rendered.extend_from_slice(&output.stdout);
    rendered.extend_from_slice(b"\x1b[0m\x1b8");
    Ok(rendered)
}
