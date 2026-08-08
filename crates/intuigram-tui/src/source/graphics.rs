use std::io::{self, Write};

use intuigram_app::{ChatId, InlineImage, MessageId};
use rasterm::{CellSize, Environment, Image, Multiplexer, Placement, Protocol, Renderer};
use ratatui::buffer::Buffer;
use ratatui::style::Color;
pub(crate) type GraphicsProtocol = Protocol;
pub(crate) type GraphicsRequest = Placement;

pub(crate) struct GraphicsFrame {
    protocol: GraphicsProtocol,
    multiplexer: Multiplexer,
    requests: Vec<GraphicsRequest>,
}

pub(crate) struct GraphicsState {
    renderer: Renderer,
}

impl GraphicsFrame {
    pub(crate) const fn new(protocol: GraphicsProtocol, multiplexer: Multiplexer) -> Self {
        Self {
            protocol,
            multiplexer,
            requests: Vec::new(),
        }
    }

    pub(crate) const fn protocol(&self) -> GraphicsProtocol {
        self.protocol
    }

    pub(crate) fn push(&mut self, id: u32, image: &InlineImage, size: CellSize) {
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

impl GraphicsState {
    pub(crate) fn new(protocol: GraphicsProtocol) -> Self {
        Self {
            renderer: Renderer::new(protocol),
        }
    }

    pub(crate) fn sync(
        &mut self,
        writer: &mut impl Write,
        requests: &[GraphicsRequest],
    ) -> io::Result<()> {
        self.renderer
            .sync(writer, requests)
            .map_err(io::Error::other)
    }

    pub(crate) fn clear(&mut self, writer: &mut impl Write) -> io::Result<()> {
        self.renderer.clear(writer).map_err(io::Error::other)
    }
}

pub(crate) fn graphics_environment() -> (GraphicsProtocol, Multiplexer) {
    let environment = Environment::from_env();
    (environment.protocol(), environment.multiplexer)
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

const fn image_color(id: u32) -> Color {
    Color::Rgb(
        ((id >> 16) & 0xff) as u8,
        ((id >> 8) & 0xff) as u8,
        (id & 0xff) as u8,
    )
}
