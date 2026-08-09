use std::fmt;

use url::Url;

/// A validated static coordinate, stored without floating-point ambiguity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GeoPointView {
    /// Latitude in millionths of a degree.
    pub latitude_microdegrees: i32,

    /// Longitude in millionths of a degree.
    pub longitude_microdegrees: i32,
}

impl GeoPointView {
    /// Human-readable coordinates with stable precision.
    #[must_use]
    pub fn coordinates(self) -> String {
        format_microdegrees(self.latitude_microdegrees, self.longitude_microdegrees)
    }
}

/// A Telegram venue result safe to retain and replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaceView {
    /// Exact venue coordinate.
    pub point: GeoPointView,

    /// User-facing place name.
    pub title: String,

    /// User-facing street address.
    pub address: String,

    /// Telegram venue provider.
    pub provider: String,

    /// Provider-owned venue identifier.
    pub venue_id: String,

    /// Provider-owned venue category.
    pub venue_type: String,
}

/// Why a coordinate or map link could not be accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocationParseError {
    /// The input is not one of Intuigram's explicit supported forms.
    Unsupported,
    /// A coordinate component is malformed or too precise.
    InvalidCoordinate,
    /// Latitude or longitude falls outside the Earth's valid bounds.
    OutOfRange,
}

impl fmt::Display for LocationParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => formatter
                .write_str("use coordinates or a direct Apple, Google, or OpenStreetMap URL"),
            Self::InvalidCoordinate => {
                formatter.write_str("coordinates must be decimal latitude and longitude")
            }
            Self::OutOfRange => {
                formatter.write_str("latitude must be -90..90 and longitude -180..180")
            }
        }
    }
}

/// Parses explicit coordinates and direct, non-redirecting map URLs.
pub fn parse_geo_point(input: &str) -> Result<GeoPointView, LocationParseError> {
    let input = input.trim();
    if let Some(value) = input.strip_prefix("geo:") {
        return parse_pair(value.split_once('?').map_or(value, |(pair, _)| pair));
    }
    if !input.contains("://") {
        return parse_pair(input);
    }
    let url = Url::parse(input).map_err(|_| LocationParseError::Unsupported)?;
    if url.scheme() != "https" || url.username() != "" || url.password().is_some() {
        return Err(LocationParseError::Unsupported);
    }
    match url.host_str() {
        Some("maps.apple.com") => one_query_pair(&url, &["ll", "coordinate"]),
        Some("www.google.com" | "maps.google.com") => google_point(&url),
        Some("www.openstreetmap.org" | "openstreetmap.org") => osm_point(&url),
        _ => Err(LocationParseError::Unsupported),
    }
}

fn google_point(url: &Url) -> Result<GeoPointView, LocationParseError> {
    if let Some(value) = one_query_value(url, &["q", "query"])? {
        return parse_pair(&value);
    }
    let marker = url
        .path_segments()
        .and_then(|mut segments| segments.find(|segment| segment.starts_with('@')))
        .ok_or(LocationParseError::Unsupported)?;
    let mut values = marker.trim_start_matches('@').split(',');
    let pair = format!(
        "{},{}",
        values.next().ok_or(LocationParseError::InvalidCoordinate)?,
        values.next().ok_or(LocationParseError::InvalidCoordinate)?,
    );
    parse_pair(&pair)
}

fn osm_point(url: &Url) -> Result<GeoPointView, LocationParseError> {
    let latitude = one_query_value(url, &["mlat"])?;
    let longitude = one_query_value(url, &["mlon"])?;
    if let (Some(latitude), Some(longitude)) = (&latitude, &longitude) {
        return parse_components(latitude, longitude);
    }
    if latitude.is_some() || longitude.is_some() {
        return Err(LocationParseError::Unsupported);
    }
    let fragment = url.fragment().ok_or(LocationParseError::Unsupported)?;
    let values = fragment
        .strip_prefix("map=")
        .ok_or(LocationParseError::Unsupported)?
        .split('/')
        .collect::<Vec<_>>();
    if values.len() != 3 {
        return Err(LocationParseError::Unsupported);
    }
    parse_components(values[1], values[2])
}

fn one_query_pair(url: &Url, keys: &[&str]) -> Result<GeoPointView, LocationParseError> {
    let value = one_query_value(url, keys)?.ok_or(LocationParseError::Unsupported)?;
    parse_pair(&value)
}

fn one_query_value(url: &Url, keys: &[&str]) -> Result<Option<String>, LocationParseError> {
    let mut values = url
        .query_pairs()
        .filter(|(candidate, _)| keys.iter().any(|key| candidate == *key))
        .map(|(_, value)| value.into_owned());
    let value = values.next();
    if values.next().is_some() {
        return Err(LocationParseError::Unsupported);
    }
    Ok(value)
}

fn parse_pair(value: &str) -> Result<GeoPointView, LocationParseError> {
    let (latitude, longitude) = value
        .split_once(',')
        .ok_or(LocationParseError::InvalidCoordinate)?;
    if longitude.contains(',') {
        return Err(LocationParseError::InvalidCoordinate);
    }
    parse_components(latitude, longitude)
}

fn parse_components(latitude: &str, longitude: &str) -> Result<GeoPointView, LocationParseError> {
    let latitude_microdegrees = parse_microdegrees(latitude)?;
    let longitude_microdegrees = parse_microdegrees(longitude)?;
    if !(-90_000_000..=90_000_000).contains(&latitude_microdegrees)
        || !(-180_000_000..=180_000_000).contains(&longitude_microdegrees)
    {
        return Err(LocationParseError::OutOfRange);
    }
    Ok(GeoPointView {
        latitude_microdegrees,
        longitude_microdegrees,
    })
}

fn parse_microdegrees(value: &str) -> Result<i32, LocationParseError> {
    let value = value.trim();
    let (negative, unsigned) = if let Some(unsigned) = value.strip_prefix('-') {
        (true, unsigned)
    } else if let Some(unsigned) = value.strip_prefix('+') {
        (false, unsigned)
    } else {
        (false, value)
    };
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty()
        || unsigned.starts_with('+')
        || unsigned.starts_with('-')
        || (unsigned.contains('.') && fraction.is_empty())
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 6
    {
        return Err(LocationParseError::InvalidCoordinate);
    }
    let whole = whole
        .parse::<i64>()
        .map_err(|_| LocationParseError::InvalidCoordinate)?;
    let fraction = format!("{fraction:0<6}")
        .parse::<i64>()
        .map_err(|_| LocationParseError::InvalidCoordinate)?;
    let magnitude = whole
        .checked_mul(1_000_000)
        .and_then(|value| value.checked_add(fraction))
        .ok_or(LocationParseError::OutOfRange)?;
    let signed = if negative { -magnitude } else { magnitude };
    i32::try_from(signed).map_err(|_| LocationParseError::OutOfRange)
}

fn format_microdegrees(latitude: i32, longitude: i32) -> String {
    fn one(value: i32) -> String {
        let sign = if value < 0 { "-" } else { "" };
        let magnitude = i64::from(value).abs();
        format!(
            "{sign}{}.{:06}",
            magnitude / 1_000_000,
            magnitude % 1_000_000
        )
    }
    format!("{}, {}", one(latitude), one(longitude))
}
