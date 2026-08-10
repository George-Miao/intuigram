use super::*;

pub(super) fn parse_media_maintenance(
    arguments: &mut impl Iterator<Item = String>,
    argument: &str,
) -> Result<RichMediaMaintenance> {
    match argument {
        "--media-browse" => Ok(RichMediaMaintenance::Browse {
            kind: library_kind(argument, next_argument(arguments, argument)?)?,
            query: query(next_argument(arguments, argument)?),
        }),
        "--media-send" => Ok(RichMediaMaintenance::SendLibrary {
            chat: chat(argument, next_argument(arguments, argument)?)?,
            kind: library_kind(argument, next_argument(arguments, argument)?)?,
            index: number(argument, next_argument(arguments, argument)?)?,
            query: query(next_argument(arguments, argument)?),
        }),
        "--media-file" => Ok(RichMediaMaintenance::SendFile {
            chat: chat(argument, next_argument(arguments, argument)?)?,
            kind: upload_kind(argument, next_argument(arguments, argument)?)?,
            path: next_argument(arguments, argument)?.into(),
        }),
        "--record-media" => {
            let chat = chat(argument, next_argument(arguments, argument)?)?;
            let kind = upload_kind(argument, next_argument(arguments, argument)?)?;
            if !matches!(kind, UploadKind::Voice | UploadKind::VideoNote) {
                return InvalidArgumentValueSnafu {
                    argument: argument.to_owned(),
                    value: "recording kind must be voice or video-note".to_owned(),
                }
                .fail();
            }
            Ok(RichMediaMaintenance::Record {
                chat,
                kind,
                seconds: number(argument, next_argument(arguments, argument)?)?,
                device: next_argument(arguments, argument)?,
            })
        }
        "--send-contact" => Ok(RichMediaMaintenance::Contact {
            chat: chat(argument, next_argument(arguments, argument)?)?,
            phone: next_argument(arguments, argument)?,
            first_name: next_argument(arguments, argument)?,
            last_name: next_argument(arguments, argument)?,
        }),
        _ => UnknownArgumentSnafu {
            argument: argument.to_owned(),
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
