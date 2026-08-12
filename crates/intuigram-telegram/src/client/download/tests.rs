use grammers_tl_types as tl;
use intuigram_lib::{MediaLocator, MediaSource, MediaThumbnail};

use super::location::{download_locator, largest_photo_size};
use super::transfer::{
    DOWNLOAD_PART_BYTES, DOWNLOAD_PART_CONCURRENCY, MAX_IN_FLIGHT_BYTES_PER_FILE, download_parts,
};

#[test]
fn preview_selection_uses_the_largest_thumbnail_within_the_limit() {
    let sizes = [photo_size("m", 512_000), photo_size("x", 12_000_000)];

    assert_eq!(
        largest_photo_size(&sizes, 8 * 1024 * 1024),
        Some(("m".to_owned(), 512_000))
    );
}

#[test]
fn preview_selection_rejects_oversized_and_invalid_sizes() {
    let sizes = [photo_size("bad", -1), photo_size("x", 12_000_000)];

    assert_eq!(largest_photo_size(&sizes, 8 * 1024 * 1024), None);
}

#[test]
fn normalized_document_location_selects_a_bounded_thumbnail_without_message_lookup() {
    let locator = MediaLocator {
        dc_id: 4,
        source: MediaSource::Document {
            id: 42,
            access_hash: 7,
            file_reference: vec![1, 2, 3],
        },
        name: "detail.webp".to_owned(),
        mime_type: "image/webp".to_owned(),
        size: 12_000_000,
        thumbnails: vec![
            MediaThumbnail {
                kind: "m".to_owned(),
                size: 512_000,
            },
            MediaThumbnail {
                kind: "x".to_owned(),
                size: 9_000_000,
            },
        ],
    };

    let selected = download_locator(&locator, Some(8 * 1024 * 1024))
        .expect("the bounded thumbnail should be selected");

    assert_eq!(selected.dc_id, 4);
    assert_eq!(selected.size, 512_000);
    assert_eq!(selected.mime_type, "image/jpeg");
}

#[test]
fn known_downloads_are_split_into_aligned_bounded_parts() {
    let part = DOWNLOAD_PART_BYTES as usize;

    assert_eq!(
        download_parts(part * 2 + 17).expect("fixture size should be valid"),
        vec![
            (0, 0, DOWNLOAD_PART_BYTES),
            (1, i64::from(DOWNLOAD_PART_BYTES), DOWNLOAD_PART_BYTES),
            (2, i64::from(DOWNLOAD_PART_BYTES) * 2, DOWNLOAD_PART_BYTES,),
        ]
    );
}

#[test]
fn concurrent_parts_have_an_explicit_per_file_byte_budget() {
    assert_eq!(
        DOWNLOAD_PART_CONCURRENCY * DOWNLOAD_PART_BYTES as usize,
        MAX_IN_FLIGHT_BYTES_PER_FILE
    );
}

fn photo_size(kind: &str, size: i32) -> tl::enums::PhotoSize {
    tl::types::PhotoSize {
        r#type: kind.to_owned(),
        w: 1,
        h: 1,
        size,
    }
    .into()
}
