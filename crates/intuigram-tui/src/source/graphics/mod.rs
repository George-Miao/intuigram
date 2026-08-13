mod worker;

use intuigram_lib::{AttachmentId, ChatId, InlineImage, MessageId};
use rasterm::{
    CellPixels, CellSize, Environment, Image, ImageId, Multiplexer, Placement, Protocol,
};
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use snafu::Snafu;
pub(crate) use worker::GraphicsWorker;

pub(crate) type GraphicsProtocol = Protocol;
pub(crate) type GraphicsRequest = Placement;

/// Background terminal-graphics preparation failure.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    /// The dedicated graphics worker thread could not be started.
    #[snafu(display("failed to start terminal graphics worker"))]
    StartWorker { source: std::io::Error },

    /// The graphics worker stopped before accepting or returning a frame.
    #[snafu(display("terminal graphics worker stopped unexpectedly"))]
    WorkerStopped,

    /// A terminal graphics frame could not be prepared.
    #[snafu(display("failed to prepare terminal graphics frame"))]
    Prepare { source: rasterm::Error },

    /// Prepared terminal graphics bytes could not be written.
    #[snafu(display("failed to write prepared terminal graphics frame"))]
    Write { source: std::io::Error },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) struct GraphicsFrame {
    protocol: GraphicsProtocol,
    multiplexer: Multiplexer,
    cell_pixels: CellPixels,
    requests: Vec<GraphicsRequest>,
}

impl GraphicsFrame {
    pub(crate) const fn new(protocol: GraphicsProtocol, multiplexer: Multiplexer) -> Self {
        Self {
            protocol,
            multiplexer,
            cell_pixels: CellPixels {
                width: 8,
                height: 16,
            },
            requests: Vec::new(),
        }
    }

    pub(crate) const fn protocol(&self) -> GraphicsProtocol {
        self.protocol
    }

    pub(crate) fn square_columns(&self, rows: u16) -> u16 {
        let width = u32::from(self.cell_pixels.width.max(1));
        let height = u32::from(self.cell_pixels.height.max(1));
        let columns = u32::from(rows).saturating_mul(height).div_ceil(width);
        u16::try_from(columns.clamp(1, u32::from(u16::MAX)))
            .expect("clamped square avatar width fits a terminal cell count")
    }

    pub(crate) fn push(&mut self, id: ImageId, image: &InlineImage, size: CellSize) {
        let image = Image::from_rgba(
            u32::from(image.width()),
            u32::from(image.height()),
            image.rgba().to_vec(),
        )
        .expect("an Intuigram InlineImage has already validated its RGBA dimensions");
        self.requests.push(GraphicsRequest {
            id,
            image: image.into(),
            size,
            cell_pixels: self.cell_pixels,
            x: 0,
            y: 0,
            multiplexer: self.multiplexer,
        });
    }

    pub(crate) fn requests(&self) -> &[GraphicsRequest] {
        &self.requests
    }

    pub(crate) fn set_multiplexer(&mut self, multiplexer: Multiplexer) {
        self.multiplexer = multiplexer;
        for request in &mut self.requests {
            request.multiplexer = multiplexer;
        }
    }

    pub(crate) fn set_cell_pixels(&mut self, cell_pixels: CellPixels) {
        self.cell_pixels = cell_pixels;
        for request in &mut self.requests {
            request.cell_pixels = cell_pixels;
        }
    }

    pub(crate) fn locate(&mut self, buffer: &Buffer) {
        self.requests.retain_mut(|request| {
            let foreground = image_color(request.id);
            for y in buffer.area.top()..buffer.area.bottom() {
                for x in buffer.area.left()..buffer.area.right() {
                    if buffer[(x, y)].fg == foreground {
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

pub(crate) fn graphics_environment() -> (GraphicsProtocol, Multiplexer) {
    let environment = Environment::from_env();
    (environment.protocol(), environment.multiplexer)
}

pub(crate) fn image_id(chat: ChatId, message: MessageId) -> ImageId {
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
    ImageId::new((hash & 0x00ff_ffff).max(1))
        .expect("masking and clamping an image hash always produces a nonzero ID")
}

pub(crate) fn attachment_image_id(attachment: AttachmentId) -> ImageId {
    let mut hash = 2_166_136_261_u32 ^ 0x4154_5443;
    for byte in attachment.0.to_le_bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    ImageId::new((hash & 0x00ff_ffff).max(1))
        .expect("masking and clamping an attachment hash always produces a nonzero ID")
}

pub(crate) fn avatar_image_id(peer: ChatId, placement: i64) -> ImageId {
    let mut hash = 2_166_136_261_u32 ^ 0x4156_4154;
    for byte in peer
        .0
        .to_le_bytes()
        .into_iter()
        .chain(placement.to_le_bytes())
    {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    ImageId::new((hash & 0x00ff_ffff).max(1))
        .expect("masking and clamping an avatar hash always produces a nonzero ID")
}

pub(crate) const fn image_color(id: ImageId) -> Color {
    let id = id.get();
    Color::Rgb(
        ((id >> 16) & 0xff) as u8,
        ((id >> 8) & 0xff) as u8,
        (id & 0xff) as u8,
    )
}
