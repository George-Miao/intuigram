use super::*;

pub(super) fn normalize_web_page(webpage: &tl::enums::WebPage) -> MediaCard {
    match webpage {
        tl::enums::WebPage::Page(page) => full_page(page),
        tl::enums::WebPage::Pending(page) => card(
            MediaKind::LinkPreview,
            "Link preview",
            page.url
                .clone()
                .unwrap_or_else(|| "URL unavailable".to_owned()),
            vec!["preview pending".to_owned()],
            Some(page.id.to_string()),
        ),
        tl::enums::WebPage::Empty(page) => card(
            MediaKind::LinkPreview,
            "Link preview",
            page.url
                .clone()
                .unwrap_or_else(|| "URL unavailable".to_owned()),
            Vec::new(),
            Some(page.id.to_string()),
        ),
        tl::enums::WebPage::NotModified(_) => card(
            MediaKind::LinkPreview,
            "Link preview",
            "preview metadata unchanged",
            Vec::new(),
            None,
        ),
    }
}

fn full_page(page: &tl::types::WebPage) -> MediaCard {
    let title = page
        .title
        .clone()
        .or_else(|| page.site_name.clone())
        .unwrap_or_else(|| "Link preview".to_owned());
    let description = page
        .description
        .clone()
        .or_else(|| page.author.clone())
        .unwrap_or_else(|| page.display_url.clone());
    let mut details = vec![page.url.clone()];
    if let Some(site) = page.site_name.as_ref().filter(|site| **site != title) {
        details.push(site.clone());
    }
    card(
        MediaKind::LinkPreview,
        title,
        description,
        details,
        Some(page.id.to_string()),
    )
}
