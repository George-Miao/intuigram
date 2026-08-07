use super::*;

pub(super) fn serialize_entities(
    entities: Vec<TextEntity>,
) -> Result<Option<Vec<tl::enums::MessageEntity>>> {
    let entities = entities
        .into_iter()
        .map(serialize_entity)
        .collect::<Result<Vec<_>>>()?;
    Ok((!entities.is_empty()).then_some(entities))
}

fn serialize_entity(entity: TextEntity) -> Result<tl::enums::MessageEntity> {
    let offset = i32::try_from(entity.offset).map_err(|_| Error::InvalidEntityRange {
        offset: entity.offset,
        length: entity.length,
    })?;
    let length = i32::try_from(entity.length).map_err(|_| Error::InvalidEntityRange {
        offset: entity.offset,
        length: entity.length,
    })?;
    Ok(match entity.kind {
        TextEntityKind::Bold => tl::types::MessageEntityBold { offset, length }.into(),
        TextEntityKind::Italic => tl::types::MessageEntityItalic { offset, length }.into(),
        TextEntityKind::Underline => tl::types::MessageEntityUnderline { offset, length }.into(),
        TextEntityKind::Strike => tl::types::MessageEntityStrike { offset, length }.into(),
        TextEntityKind::Code => tl::types::MessageEntityCode { offset, length }.into(),
        TextEntityKind::Pre { language } => tl::types::MessageEntityPre {
            offset,
            length,
            language: language.unwrap_or_default(),
        }
        .into(),
        TextEntityKind::Spoiler => tl::types::MessageEntitySpoiler { offset, length }.into(),
        TextEntityKind::Url => tl::types::MessageEntityUrl { offset, length }.into(),
        TextEntityKind::TextUrl { url } => tl::types::MessageEntityTextUrl {
            offset,
            length,
            url,
        }
        .into(),
        TextEntityKind::CustomEmoji { document_id } => tl::types::MessageEntityCustomEmoji {
            offset,
            length,
            document_id,
        }
        .into(),
        TextEntityKind::Semantic => tl::types::MessageEntityUnknown { offset, length }.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_entities_keep_their_utf16_ranges_at_the_tl_boundary() {
        let entities = serialize_entities(vec![TextEntity {
            offset: 3,
            length: 4,
            kind: TextEntityKind::Bold,
        }])
        .expect("a small entity range should serialize")
        .expect("a non-empty entity list should remain present");

        assert!(matches!(
            &entities[0],
            tl::enums::MessageEntity::Bold(entity)
                if entity.offset == 3 && entity.length == 4
        ));
    }
}
