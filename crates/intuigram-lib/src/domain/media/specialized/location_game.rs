/// A live location normalized away from Telegram constructors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveLocationView {
    /// Latitude in millionths of a degree.
    pub latitude_microdegrees: i32,

    /// Longitude in millionths of a degree.
    pub longitude_microdegrees: i32,

    /// Travel heading clockwise from north, when Telegram provides it.
    pub heading_degrees: Option<u16>,

    /// Total sharing period requested by the sender.
    pub period_seconds: u32,

    /// Radius that triggers a proximity notification, when configured.
    pub proximity_radius_metres: Option<u32>,

    /// Telegram-reported coordinate accuracy radius, when available.
    pub accuracy_radius_metres: Option<u32>,
}

impl LiveLocationView {
    /// Human-readable coordinates suitable for the text fallback.
    #[must_use]
    pub fn coordinates(&self) -> String {
        format!(
            "{:.6}, {:.6}",
            f64::from(self.latitude_microdegrees) / 1_000_000.0,
            f64::from(self.longitude_microdegrees) / 1_000_000.0,
        )
    }

    /// Stable HTTPS map target safe to hand to the platform launcher.
    #[must_use]
    pub fn map_url(&self) -> String {
        format!(
            "https://www.openstreetmap.org/?mlat={:.6}&mlon={:.6}#map=16/{:.6}/{:.6}",
            f64::from(self.latitude_microdegrees) / 1_000_000.0,
            f64::from(self.longitude_microdegrees) / 1_000_000.0,
            f64::from(self.latitude_microdegrees) / 1_000_000.0,
            f64::from(self.longitude_microdegrees) / 1_000_000.0,
        )
    }
}

/// Telegram game metadata that remains meaningful without launching a bot
/// webview.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameView {
    /// Stable Telegram game identifier.
    pub id: i64,

    /// Bot-defined short name used to identify the game.
    pub short_name: String,

    /// User-facing game title.
    pub title: String,

    /// User-facing game description.
    pub description: String,
}
