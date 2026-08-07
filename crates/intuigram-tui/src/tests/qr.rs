use super::*;

#[test]
fn qr_login_renderer_produces_a_compact_high_contrast_symbol() {
    let rendered = qr_login_symbols("tg://login?token=-_8").expect("login URI should fit a QR");
    let lines = rendered.dense.lines().collect::<Vec<_>>();

    assert!(lines.len() > 10);
    assert!(lines.len() < 30);
    assert!(lines.iter().any(|line| line.contains('█')));
    assert!(lines.iter().all(|line| line.chars().count() > 20));
}

#[test]
fn full_size_login_token_has_an_80_by_24_terminal_fallback() {
    let uri = format!("tg://login?token={}", "a".repeat(350));
    let rendered = qr_login_symbols(&uri).expect("login URI should fit a QR");
    let lines = rendered.compact.lines().collect::<Vec<_>>();

    assert!(lines.len() <= 20);
    assert!(
        lines
            .iter()
            .all(|line| line.chars().count() <= usize::from(80_u16))
    );
    assert!(
        lines
            .iter()
            .any(|line| line.chars().any(|ch| ch > '\u{2800}'))
    );
}
