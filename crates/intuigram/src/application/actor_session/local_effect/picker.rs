use std::process::Stdio;

use compio::process::Command;
use intuigram_config::ExternalCommand;
use snafu::{OptionExt, ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub(super) enum Error {
    #[snafu(display("failed to run external path picker {program:?}"))]
    Run {
        program: String,
        source: std::io::Error,
    },

    #[snafu(display("external path picker {program:?} exited with {status}"))]
    Failed {
        program: String,
        status: std::process::ExitStatus,
    },

    #[snafu(display("external path picker output is not valid UTF-8"))]
    InvalidOutput { source: std::string::FromUtf8Error },

    #[snafu(display("external path picker did not select a path"))]
    NoSelection,
}

type Result<T, E = Error> = std::result::Result<T, E>;

pub(super) async fn select(command: Option<ExternalCommand>) -> Result<Option<String>> {
    let Some(command) = command else {
        return Ok(None);
    };
    let mut process = Command::new(&command.program);
    process.args(command.args);
    process
        .stdin(Stdio::null())
        .expect("std::process::Stdio conversion is infallible");
    process
        .stdout(Stdio::piped())
        .expect("std::process::Stdio conversion is infallible");
    process
        .stderr(Stdio::null())
        .expect("std::process::Stdio conversion is infallible");
    let output = process.output().await.context(RunSnafu {
        program: command.program.clone(),
    })?;
    if !output.status.success() {
        return FailedSnafu {
            program: command.program,
            status: output.status,
        }
        .fail();
    }
    let output = String::from_utf8(output.stdout).context(InvalidOutputSnafu)?;
    output
        .lines()
        .map(str::trim)
        .find(|path| !path.is_empty())
        .map(str::to_owned)
        .context(NoSelectionSnafu)
        .map(Some)
}
