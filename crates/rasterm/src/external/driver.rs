use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use snafu::{OptionExt, ResultExt};

use super::{
    MissingPipeSnafu, Result, SpawnSnafu, UeberzugCommand, WriteImageSnafu, WriteSnafu, chafa, png,
};
use crate::{ImageId, Placement, Protocol};

const UEBERZUG: &str = "ueberzugpp";
static NEXT_FILE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub(crate) enum Driver {
    None,
    Chafa,
    Ueberzug(UeberzugLayer),
}

impl Driver {
    pub(crate) fn new(protocol: Protocol) -> Self {
        match protocol {
            Protocol::Chafa => Self::Chafa,
            Protocol::Ueberzug => Self::Ueberzug(UeberzugLayer::new()),
            Protocol::Text
            | Protocol::KittyUnicode
            | Protocol::KittyLegacy
            | Protocol::Iterm2
            | Protocol::Sixel => Self::None,
        }
    }

    pub(crate) fn place(&mut self, placement: &Placement) -> Result<Vec<u8>> {
        match self {
            Self::Chafa => chafa::render(placement),
            Self::Ueberzug(layer) => {
                layer.place(placement)?;
                Ok(Vec::new())
            }
            Self::None => Ok(Vec::new()),
        }
    }

    pub(crate) fn delete(&mut self, id: ImageId) -> Result<()> {
        if let Self::Ueberzug(layer) = self {
            layer.delete(id)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct UeberzugLayer {
    process: Option<Child>,
    input: Option<ChildStdin>,
    images: HashMap<ImageId, std::path::PathBuf>,
}

impl UeberzugLayer {
    fn new() -> Self {
        Self {
            process: None,
            input: None,
            images: HashMap::new(),
        }
    }

    fn place(&mut self, placement: &Placement) -> Result<()> {
        self.start()?;
        let path = private_png_path(placement.id);
        std::fs::write(&path, png(&placement.image)?)
            .context(WriteImageSnafu { path: path.clone() })?;
        self.write(UeberzugCommand::add(
            placement.id,
            &path,
            placement.x,
            placement.y,
            placement.size,
        ))?;
        if let Some(previous) = self.images.insert(placement.id, path) {
            let _ = std::fs::remove_file(previous);
        }
        Ok(())
    }

    fn delete(&mut self, id: ImageId) -> Result<()> {
        if self.input.is_some() {
            self.write(UeberzugCommand::remove(id))?;
        }
        if let Some(path) = self.images.remove(&id) {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        if self.input.is_some() {
            return Ok(());
        }
        let mut process = Command::new(UEBERZUG)
            .args(["layer", "--parser", "json"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context(SpawnSnafu { program: UEBERZUG })?;
        let input = process.stdin.take().context(MissingPipeSnafu {
            program: UEBERZUG,
            pipe: "stdin",
        })?;
        self.process = Some(process);
        self.input = Some(input);
        Ok(())
    }

    fn write(&mut self, command: UeberzugCommand) -> Result<()> {
        self.input
            .as_mut()
            .context(MissingPipeSnafu {
                program: UEBERZUG,
                pipe: "stdin",
            })?
            .write_all(command.json_line().as_bytes())
            .context(WriteSnafu { program: UEBERZUG })
    }
}

impl Drop for UeberzugLayer {
    fn drop(&mut self) {
        self.input.take();
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
        for path in self.images.drain().map(|(_, path)| path) {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn private_png_path(id: ImageId) -> std::path::PathBuf {
    let serial = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "rasterm-{}-{}-{serial}.png",
        std::process::id(),
        id.get()
    ))
}
