use super::*;

const SMALL_DEFAULT: usize = 5;
const LARGE_DEFAULT: usize = 2;

/// Telegram-advertised per-data-center media admission limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaLimits {
    /// Maximum simultaneously active files below Telegram's 20 MiB boundary.
    pub small: usize,

    /// Maximum simultaneously active files at or above the boundary.
    pub large: usize,
}

impl Default for MediaLimits {
    fn default() -> Self {
        Self {
            small: SMALL_DEFAULT,
            large: LARGE_DEFAULT,
        }
    }
}

pub(crate) fn normalize(settings: tl::enums::account::AutoDownloadSettings) -> MediaLimits {
    let tl::enums::account::AutoDownloadSettings::Settings(settings) = settings;
    let tl::enums::AutoDownloadSettings::Settings(high) = settings.high;
    MediaLimits {
        small: positive(high.small_queue_active_operations_max).unwrap_or(SMALL_DEFAULT),
        large: positive(high.large_queue_active_operations_max).unwrap_or(LARGE_DEFAULT),
    }
}

fn positive(value: i32) -> Option<usize> {
    usize::try_from(value).ok().filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertised_limits_replace_safe_defaults() {
        let limits = normalize(settings(9, 4));

        assert_eq!(limits, MediaLimits { small: 9, large: 4 });
    }

    #[test]
    fn invalid_limits_keep_safe_defaults() {
        assert_eq!(normalize(settings(0, -1)), MediaLimits::default());
    }

    fn settings(small: i32, large: i32) -> tl::enums::account::AutoDownloadSettings {
        let preset = || {
            tl::types::AutoDownloadSettings {
                disabled: false,
                video_preload_large: false,
                audio_preload_next: false,
                phonecalls_less_data: false,
                stories_preload: false,
                photo_size_max: 0,
                video_size_max: 0,
                file_size_max: 0,
                video_upload_maxbitrate: 0,
                small_queue_active_operations_max: small,
                large_queue_active_operations_max: large,
            }
            .into()
        };
        tl::types::account::AutoDownloadSettings {
            low: preset(),
            medium: preset(),
            high: preset(),
        }
        .into()
    }
}
