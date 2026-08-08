use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::io::{self, Write};

use base64::Engine as _;
use intuigram_app::{ChatId, InlineImage, MessageId};
use ratatui::buffer::Buffer;
use ratatui::style::Color;

const RAW_CHUNK_BYTES: usize = 3_072;
pub(crate) const PLACEHOLDER: char = '\u{10eeee}';

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum GraphicsProtocol {
    #[default]
    Text,
    KittyUnicode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GraphicsRequest {
    pub(crate) id: u32,
    pub(crate) image: InlineImage,
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    pub(crate) x: u16,
    pub(crate) y: u16,
}

pub(crate) struct GraphicsFrame {
    protocol: GraphicsProtocol,
    requests: Vec<GraphicsRequest>,
}

#[derive(Default)]
pub(crate) struct GraphicsState {
    images: HashMap<u32, u64>,
}

impl GraphicsFrame {
    pub(crate) const fn new(protocol: GraphicsProtocol) -> Self {
        Self {
            protocol,
            requests: Vec::new(),
        }
    }

    pub(crate) const fn protocol(&self) -> GraphicsProtocol {
        self.protocol
    }

    pub(crate) fn push(&mut self, request: GraphicsRequest) {
        self.requests.push(request);
    }

    pub(crate) fn requests(&self) -> &[GraphicsRequest] {
        &self.requests
    }

    pub(crate) fn locate(&mut self, buffer: &Buffer) {
        self.requests.retain_mut(|request| {
            let foreground = image_color(request.id);
            for y in buffer.area.top()..buffer.area.bottom() {
                for x in buffer.area.left()..buffer.area.right() {
                    let cell = &buffer[(x, y)];
                    if cell.fg == foreground && cell.symbol().starts_with(PLACEHOLDER) {
                        request.x = x;
                        request.y = y;
                        return true;
                    }
                }
            }
            false
        });
    }
}

impl GraphicsProtocol {
    pub(crate) fn from_env() -> Self {
        Self::detect(
            std::env::var_os("TERM").as_deref(),
            std::env::var_os("ZELLIJ").as_deref(),
        )
    }

    pub(crate) fn detect(term: Option<&OsStr>, zellij: Option<&OsStr>) -> Self {
        if zellij.is_some() {
            return Self::Text;
        }
        match term.and_then(OsStr::to_str) {
            Some("xterm-ghostty") => Self::KittyUnicode,
            _ => Self::Text,
        }
    }
}

impl GraphicsState {
    pub(crate) fn sync(
        &mut self,
        writer: &mut impl Write,
        requests: &[GraphicsRequest],
    ) -> io::Result<()> {
        let visible = requests
            .iter()
            .map(|request| request.id)
            .collect::<HashSet<_>>();
        let stale = self
            .images
            .keys()
            .copied()
            .filter(|id| !visible.contains(id))
            .collect::<Vec<_>>();
        for id in stale {
            writer.write_all(&encode_delete(id))?;
            self.images.remove(&id);
        }
        for request in requests {
            let fingerprint = fingerprint(request);
            if self.images.get(&request.id) == Some(&fingerprint) {
                continue;
            }
            if self.images.contains_key(&request.id) {
                writer.write_all(&encode_delete(request.id))?;
            }
            writer.write_all(&encode_upload(
                request.id,
                &request.image,
                request.x,
                request.y,
                request.columns,
                request.rows,
            ))?;
            self.images.insert(request.id, fingerprint);
        }
        writer.flush()
    }

    pub(crate) fn clear(&mut self, writer: &mut impl Write) -> io::Result<()> {
        for id in self.images.keys().copied().collect::<Vec<_>>() {
            writer.write_all(&encode_delete(id))?;
        }
        self.images.clear();
        writer.flush()
    }
}

pub(crate) fn image_id(chat: ChatId, message: MessageId) -> u32 {
    let mut hash = 2_166_136_261_u32;
    for byte in chat
        .0
        .to_le_bytes()
        .into_iter()
        .chain(message.0.to_le_bytes())
    {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    (hash & 0x00ff_ffff).max(1)
}

pub(crate) fn encode_upload(
    id: u32,
    image: &InlineImage,
    x: u16,
    y: u16,
    columns: u16,
    rows: u16,
) -> Vec<u8> {
    let chunks = image.rgba().chunks(RAW_CHUNK_BYTES).collect::<Vec<_>>();
    let mut output = Vec::new();
    write!(
        output,
        "\x1b[{};{}H",
        y.saturating_add(1),
        x.saturating_add(1),
    )
    .expect("writing a cursor command to memory cannot fail");
    for (index, chunk) in chunks.iter().enumerate() {
        let more = usize::from(index + 1 < chunks.len());
        if index == 0 {
            write!(
                output,
                "\x1b_Gq=2,a=T,C=1,f=32,s={},v={},i={id},c={columns},r={rows},m={more};",
                image.width(),
                image.height(),
            )
            .expect("writing a graphics command to memory cannot fail");
        } else {
            write!(output, "\x1b_Gm={more};")
                .expect("writing a graphics command to memory cannot fail");
        }
        output.extend_from_slice(
            base64::engine::general_purpose::STANDARD
                .encode(chunk)
                .as_bytes(),
        );
        output.extend_from_slice(b"\x1b\\");
    }
    output
}

fn encode_delete(id: u32) -> Vec<u8> {
    format!("\x1b_Ga=d,d=I,i={id},q=2\x1b\\").into_bytes()
}

fn fingerprint(request: &GraphicsRequest) -> u64 {
    let mut hash = 14_695_981_039_346_656_037_u64;
    for byte in request
        .image
        .width()
        .to_le_bytes()
        .into_iter()
        .chain(request.image.height().to_le_bytes())
        .chain(request.x.to_le_bytes())
        .chain(request.y.to_le_bytes())
        .chain(request.columns.to_le_bytes())
        .chain(request.rows.to_le_bytes())
        .chain(request.image.rgba().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}

const fn image_color(id: u32) -> Color {
    Color::Rgb(
        ((id >> 16) & 0xff) as u8,
        ((id >> 8) & 0xff) as u8,
        (id & 0xff) as u8,
    )
}
