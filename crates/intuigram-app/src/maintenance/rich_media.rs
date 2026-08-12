use super::*;

pub(crate) async fn run_rich_media_maintenance(
    config: &Config,
    config_directory: &std::path::Path,
    account: AccountId,
    command: RichMediaMaintenance,
) -> Result<()> {
    let mut client = connect_account(config, config_directory, account).await?;
    match command {
        RichMediaMaintenance::Browse { kind, query } => {
            for (index, entry) in client
                .browse_media(kind, &query, 50)
                .await
                .context(TelegramSnafu)?
                .iter()
                .enumerate()
            {
                println!("{index}\t{}\t{}", entry.id, entry.label);
            }
        }
        RichMediaMaintenance::SendLibrary {
            chat,
            kind,
            index,
            query,
        } => {
            let entries = client
                .browse_media(kind, &query, index.saturating_add(1))
                .await
                .context(TelegramSnafu)?;
            let entry = entries
                .get(index)
                .context(MediaIndexUnavailableSnafu { index })?;
            client
                .send_library_media(chat, entry, None, None, None, operation_id()?)
                .await
                .context(TelegramSnafu)?;
            println!("Sent {} to Chat {}.", entry.label, chat.0);
        }
        RichMediaMaintenance::SendFile { chat, kind, path } => {
            send_file(&mut client, chat, kind, path).await?;
        }
        RichMediaMaintenance::Record {
            chat,
            kind,
            seconds,
            device,
        } => {
            let path = record_media(kind, seconds, &device).await?;
            let sent = send_file(&mut client, chat, kind, path.clone()).await;
            let _ = std::fs::remove_file(path);
            sent?;
        }
        RichMediaMaintenance::Contact {
            chat,
            phone,
            first_name,
            last_name,
        } => {
            client
                .send_contact(intuigram_telegram::ContactCardSend {
                    chat,
                    phone_number: phone,
                    first_name,
                    last_name,
                    reply_to: None,
                    thread_root: None,
                    monoforum_peer: None,
                    random_id: operation_id()?,
                })
                .await
                .context(TelegramSnafu)?;
            println!("Sent a contact card to Chat {}.", chat.0);
        }
    }
    Ok(())
}

async fn send_file(
    client: &mut Client,
    chat: ChatId,
    kind: UploadKind,
    path: PathBuf,
) -> Result<()> {
    let bytes = compio::fs::read(&path)
        .await
        .context(ReadAttachmentSnafu { path: path.clone() })?;
    let name = path.file_name().map_or_else(
        || "media".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let id = operation_id()?;
    client
        .send_upload(intuigram_telegram::UploadSend {
            chat,
            upload: intuigram_telegram::Upload {
                name,
                mime_type: mime_type_for_path(&path),
                bytes,
                kind,
            },
            caption: String::new(),
            entities: Vec::new(),
            reply_to: None,
            thread_root: None,
            monoforum_peer: None,
            ids: intuigram_telegram::UploadIds {
                file: derived_random_id(id, 0, 0x4649_4c45),
                message: derived_random_id(id, 0, 0x4d45_5353),
            },
        })
        .await
        .context(TelegramSnafu)?;
    println!("Sent media to Chat {}.", chat.0);
    Ok(())
}

pub(crate) async fn record_media(kind: UploadKind, seconds: u32, device: &str) -> Result<PathBuf> {
    let label = match kind {
        UploadKind::Voice => "voice",
        UploadKind::VideoNote => "video note",
        _ => unreachable!("the argument parser limits recording kinds"),
    };
    let extension = if kind == UploadKind::Voice {
        "ogg"
    } else {
        "mp4"
    };
    let path = std::env::temp_dir().join(format!(
        "intuigram-recording-{}.{}",
        operation_id()?.unsigned_abs(),
        extension
    ));
    let mut command = compio::process::Command::new("ffmpeg");
    command.args(["-nostdin", "-y", "-loglevel", "error"]);
    add_capture_input(&mut command, kind, device);
    command.args(["-t", &seconds.max(1).to_string()]);
    if kind == UploadKind::Voice {
        command.args(["-c:a", "libopus", "-b:a", "32k"]);
    } else {
        command.args([
            "-vf",
            "crop=min(iw\\,ih):min(iw\\,ih),scale=480:480",
            "-c:v",
            "libx264",
            "-an",
            "-movflags",
            "+faststart",
        ]);
    }
    command.arg(&path);
    let status = command
        .status()
        .await
        .context(RecordMediaSnafu { kind: label })?;
    if !status.success() {
        return RecorderFailedSnafu {
            kind: label,
            status,
        }
        .fail();
    }
    Ok(path)
}

fn add_capture_input(command: &mut compio::process::Command, kind: UploadKind, device: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = kind;
        command.args(["-f", "avfoundation", "-i", device]);
    }
    #[cfg(target_os = "linux")]
    if kind == UploadKind::Voice {
        command.args(["-f", "pulse", "-i", device]);
    } else {
        command.args(["-f", "v4l2", "-i", device]);
    }
    #[cfg(target_os = "windows")]
    command.args([
        "-f",
        "dshow",
        "-i",
        &format!(
            "{}={device}",
            if kind == UploadKind::Voice {
                "audio"
            } else {
                "video"
            }
        ),
    ]);
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let _ = (command, kind, device);
}

fn operation_id() -> Result<i64> {
    let mut bytes = [0_u8; 8];
    getrandom::fill(&mut bytes).context(OperationIdSnafu)?;
    Ok(i64::from_le_bytes(bytes))
}
