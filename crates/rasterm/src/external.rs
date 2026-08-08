use std::path::{Path, PathBuf};

use crate::CellSize;

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
        Self {
            program: "chafa",
            arguments: vec![
                "--format=symbols".to_owned(),
                format!("--size={}x{}", size.columns, size.rows),
                "--".to_owned(),
                path.to_string_lossy().into_owned(),
            ],
        }
    }
}

/// JSON-line command for a caller-owned asynchronous Überzug++ layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UeberzugCommand {
    /// Adds or replaces one overlay.
    Add {
        /// Stable overlay name.
        identifier: String,

        /// Encoded image file owned by the caller.
        path: PathBuf,

        /// Terminal column.
        x: u16,

        /// Terminal row.
        y: u16,

        /// Maximum cell size.
        size: CellSize,
    },

    /// Removes one overlay.
    Remove {
        /// Stable overlay name.
        identifier: String,
    },
}

impl UeberzugCommand {
    /// Encodes one command for `ueberzugpp layer --parser json`.
    #[must_use]
    pub fn json_line(&self) -> String {
        match self {
            Self::Add {
                identifier,
                path,
                x,
                y,
                size,
            } => format!(
                "{{\"action\":\"add\",\"identifier\":\"{}\",\"path\":\"{}\",\"x\":{x},\"y\":{y},\"\
                 max_width\":{},\"max_height\":{}}}\n",
                escape(identifier),
                escape(&path.to_string_lossy()),
                size.columns,
                size.rows,
            ),
            Self::Remove { identifier } => format!(
                "{{\"action\":\"remove\",\"identifier\":\"{}\"}}\n",
                escape(identifier)
            ),
        }
    }
}

fn escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            character => vec![character],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{ChafaCommand, UeberzugCommand};
    use crate::CellSize;

    const SIZE: CellSize = CellSize {
        columns: 12,
        rows: 6,
    };

    #[test]
    fn external_renderers_receive_shell_free_requests() {
        let chafa = ChafaCommand::symbols(Path::new("/tmp/a b.png"), SIZE);
        assert_eq!(chafa.program, "chafa");
        assert_eq!(
            chafa.arguments.last().map(String::as_str),
            Some("/tmp/a b.png")
        );

        let command = UeberzugCommand::Add {
            identifier: "message-42".to_owned(),
            path: "/tmp/a b.png".into(),
            x: 7,
            y: 9,
            size: SIZE,
        };
        assert_eq!(
            command.json_line(),
            "{\"action\":\"add\",\"identifier\":\"message-42\",\"path\":\"/tmp/a \
             b.png\",\"x\":7,\"y\":9,\"max_width\":12,\"max_height\":6}\n"
        );
    }
}
