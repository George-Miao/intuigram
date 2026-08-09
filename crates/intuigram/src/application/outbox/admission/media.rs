use intuigram::OperationStamp;
use intuigram_app::Effect;
use intuigram_store::OutboxAdmission;

use super::super::model::Command;
use super::super::model::send::{Contact, LibraryMedia, MessageSend, Venue};
use super::super::model::shared::GeoPoint;
use super::input::PreparedInputs;
use super::message::{destination, finish, outgoing};
use super::{Error, Result, conversion};

pub(super) fn prepare(
    effect: &Effect,
    stamp: OperationStamp,
    mut inputs: PreparedInputs,
) -> Result<OutboxAdmission> {
    let (destination, local_id, reply_to, command, media, body) = match effect {
        Effect::SendLibraryMedia {
            chat,
            local_id,
            reply_to,
            thread_root,
            saved_peer,
            ..
        } => {
            let entry = inputs.library()?;
            let content = LibraryMedia {
                kind: conversion::library_kind(entry.kind()),
                document_id: entry.id,
                access_hash: entry.access_hash(),
                file_reference: entry.file_reference().to_vec(),
            };
            (
                destination(*chat, *thread_root, *saved_peer),
                *local_id,
                *reply_to,
                Command::Library(MessageSend::new(
                    local_id.0,
                    reply_to.map(|id| id.0),
                    content,
                )),
                Vec::new(),
                entry.label,
            )
        }
        Effect::SendRichMediaFile {
            chat,
            local_id,
            reply_to,
            thread_root,
            saved_peer,
            ..
        }
        | Effect::RecordRichMedia {
            chat,
            local_id,
            reply_to,
            thread_root,
            saved_peer,
            ..
        } => {
            let (content, media) = inputs.rich_media()?;
            let name = media[0].file_name.clone();
            let send = MessageSend::new(local_id.0, reply_to.map(|id| id.0), content);
            let command = if matches!(effect, Effect::RecordRichMedia { .. }) {
                Command::Recording(send)
            } else {
                Command::File(send)
            };
            (
                destination(*chat, *thread_root, *saved_peer),
                *local_id,
                *reply_to,
                command,
                media,
                name,
            )
        }
        Effect::SendContact {
            chat,
            phone,
            first_name,
            last_name,
            local_id,
            reply_to,
            thread_root,
            saved_peer,
        } => (
            destination(*chat, *thread_root, *saved_peer),
            *local_id,
            *reply_to,
            Command::Contact(MessageSend::new(
                local_id.0,
                reply_to.map(|id| id.0),
                Contact {
                    phone: phone.clone(),
                    first_name: first_name.clone(),
                    last_name: last_name.clone(),
                },
            )),
            Vec::new(),
            format!("[Contact] {first_name} {last_name}"),
        ),
        Effect::SendStaticLocation {
            chat,
            point,
            local_id,
            reply_to,
            thread_root,
            saved_peer,
        } => (
            destination(*chat, *thread_root, *saved_peer),
            *local_id,
            *reply_to,
            Command::StaticLocation(MessageSend::new(
                local_id.0,
                reply_to.map(|id| id.0),
                GeoPoint {
                    latitude_microdegrees: point.latitude_microdegrees,
                    longitude_microdegrees: point.longitude_microdegrees,
                },
            )),
            Vec::new(),
            format!("[Location] {}", point.coordinates()),
        ),
        Effect::SendVenue {
            chat,
            venue,
            local_id,
            reply_to,
            thread_root,
            saved_peer,
        } => (
            destination(*chat, *thread_root, *saved_peer),
            *local_id,
            *reply_to,
            Command::Venue(MessageSend::new(
                local_id.0,
                reply_to.map(|id| id.0),
                Venue {
                    point: GeoPoint {
                        latitude_microdegrees: venue.point.latitude_microdegrees,
                        longitude_microdegrees: venue.point.longitude_microdegrees,
                    },
                    title: venue.title.clone(),
                    address: venue.address.clone(),
                    provider: venue.provider.clone(),
                    venue_id: venue.venue_id.clone(),
                    venue_type: venue.venue_type.clone(),
                },
            )),
            Vec::new(),
            format!("[{}] {}", venue.title, venue.address),
        ),
        _ => {
            return Err(Error::Incomplete {
                reason: "send operation has no admission mapping",
            });
        }
    };
    let message = outgoing(local_id, body, Vec::new(), reply_to);
    finish(
        destination,
        local_id,
        reply_to,
        command,
        media,
        message,
        stamp,
    )
}
