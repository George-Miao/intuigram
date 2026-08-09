use std::path::{Path, PathBuf};

use crate::{CellSize, ImageId};

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
    pub(crate) fn add(id: ImageId, path: &Path, x: u16, y: u16, size: CellSize) -> Self {
        Self::Add {
            identifier: identifier(id),
            path: path.to_owned(),
            x,
            y,
            size,
        }
    }

    pub(crate) fn remove(id: ImageId) -> Self {
        Self::Remove {
            identifier: identifier(id),
        }
    }

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

fn identifier(id: ImageId) -> String {
    format!("rasterm-{}", id.get())
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
