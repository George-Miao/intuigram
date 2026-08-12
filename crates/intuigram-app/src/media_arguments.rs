use crate::launch::{Error, InvalidArgumentValueSnafu, Result, UnknownArgumentSnafu};
use crate::{ChatId, MediaLibraryKind, RichMediaMaintenance, UploadKind, next_argument};

pub(super) fn parse_media_maintenance(
    arguments: &mut impl Iterator<Item = String>,
    action: &str,
    label: &str,
) -> Result<RichMediaMaintenance> {
    match action {
        "browse" => Ok(RichMediaMaintenance::Browse {
            kind: library_kind(label, next_argument(arguments, label)?)?,
            query: query(next_argument(arguments, label)?),
        }),
        "send" => Ok(RichMediaMaintenance::SendLibrary {
            chat: chat(label, next_argument(arguments, label)?)?,
            kind: library_kind(label, next_argument(arguments, label)?)?,
            index: number(label, next_argument(arguments, label)?)?,
            query: query(next_argument(arguments, label)?),
        }),
        "file" => Ok(RichMediaMaintenance::SendFile {
            chat: chat(label, next_argument(arguments, label)?)?,
            kind: upload_kind(label, next_argument(arguments, label)?)?,
            path: next_argument(arguments, label)?.into(),
        }),
        "record" => {
            let chat = chat(label, next_argument(arguments, label)?)?;
            let kind = upload_kind(label, next_argument(arguments, label)?)?;
            if !matches!(kind, UploadKind::Voice | UploadKind::VideoNote) {
                return InvalidArgumentValueSnafu {
                    argument: label.to_owned(),
                    value: "recording kind must be voice or video-note".to_owned(),
                }
                .fail();
            }
            Ok(RichMediaMaintenance::Record {
                chat,
                kind,
                seconds: number(label, next_argument(arguments, label)?)?,
                device: next_argument(arguments, label)?,
            })
        }
        "contact" => Ok(RichMediaMaintenance::Contact {
            chat: chat(label, next_argument(arguments, label)?)?,
            phone: next_argument(arguments, label)?,
            first_name: next_argument(arguments, label)?,
            last_name: next_argument(arguments, label)?,
        }),
        _ => UnknownArgumentSnafu {
            argument: label.to_owned(),
        }
        .fail(),
    }
}

fn chat(argument: &str, value: String) -> Result<ChatId> {
    value
        .parse::<i64>()
        .ok()
        .filter(|id| *id != 0)
        .map(ChatId)
        .ok_or_else(|| invalid(argument, value))
}

fn number<T>(argument: &str, value: String) -> Result<T>
where
    T: std::str::FromStr,
{
    value.parse().map_err(|_| invalid(argument, value))
}

fn library_kind(argument: &str, value: String) -> Result<MediaLibraryKind> {
    match value.as_str() {
        "sticker" | "stickers" => Ok(MediaLibraryKind::Stickers),
        "gif" | "gifs" => Ok(MediaLibraryKind::Gifs),
        "custom-emoji" => Ok(MediaLibraryKind::CustomEmoji),
        _ => Err(invalid(argument, value)),
    }
}

fn upload_kind(argument: &str, value: String) -> Result<UploadKind> {
    match value.as_str() {
        "voice" => Ok(UploadKind::Voice),
        "video-note" => Ok(UploadKind::VideoNote),
        "sticker" => Ok(UploadKind::Sticker),
        "gif" => Ok(UploadKind::Animation),
        "custom-emoji" => Ok(UploadKind::CustomEmoji),
        _ => Err(invalid(argument, value)),
    }
}

fn query(value: String) -> String {
    if value == "-" { String::new() } else { value }
}

fn invalid(argument: &str, value: String) -> Error {
    Error::InvalidArgumentValue {
        argument: argument.to_owned(),
        value,
    }
}
