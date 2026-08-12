pub(crate) struct QrLoginSymbols {
    pub(crate) dense: String,
    pub(crate) compact: String,
}

pub(crate) fn qr_login_symbols(uri: &str) -> Result<QrLoginSymbols> {
    let dense = QrCode::new(uri.as_bytes()).context(EncodeQrSnafu)?;
    let compact =
        QrCode::with_error_correction_level(uri.as_bytes(), EcLevel::L).context(EncodeQrSnafu)?;
    Ok(QrLoginSymbols {
        dense: dense
            .render::<Dense1x2>()
            .module_dimensions(1, 1)
            .quiet_zone(true)
            .build(),
        compact: render_braille_qr(&compact),
    })
}

pub(super) fn render_braille_qr(code: &QrCode) -> String {
    const QUIET_ZONE: usize = 4;
    const BRAILLE: [[u8; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];

    let source_width = code.width();
    let width = source_width + QUIET_ZONE * 2;
    let colors = code.to_colors();
    let mut rendered = String::new();
    for cell_y in (0..width).step_by(4) {
        if cell_y > 0 {
            rendered.push('\n');
        }
        for cell_x in (0..width).step_by(2) {
            let mut dots = 0_u8;
            for (dy, row) in BRAILLE.iter().enumerate() {
                for (dx, bit) in row.iter().enumerate() {
                    let x = cell_x + dx;
                    let y = cell_y + dy;
                    if x >= QUIET_ZONE
                        && y >= QUIET_ZONE
                        && x < source_width + QUIET_ZONE
                        && y < source_width + QUIET_ZONE
                        && colors[(y - QUIET_ZONE) * source_width + (x - QUIET_ZONE)]
                            == QrColor::Dark
                    {
                        dots |= 1 << bit;
                    }
                }
            }
            rendered.push(char::from_u32(0x2800 + u32::from(dots)).expect("valid Braille cell"));
        }
    }
    rendered
}

pub(in crate::source) fn render_qr_login(
    frame: &mut Frame<'_>,
    qr: &QrLoginSymbols,
    expires_in: u64,
) {
    let area = frame.area();
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "Link Intuigram to Telegram",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from("Scan in Telegram: Settings → Devices → Link Desktop Device"),
        ])
        .alignment(Alignment::Center),
        rows[0],
    );

    let symbol = [&qr.dense, &qr.compact].into_iter().find(|symbol| {
        let width = symbol
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        symbol.lines().count() <= usize::from(rows[1].height) && width <= usize::from(rows[1].width)
    });
    if let Some(symbol) = symbol {
        let qr_height = u16::try_from(symbol.lines().count()).unwrap_or(u16::MAX);
        let qr_width = u16::try_from(
            symbol
                .lines()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(0),
        )
        .unwrap_or(u16::MAX);
        let qr_area = Rect {
            x: rows[1].x + rows[1].width.saturating_sub(qr_width) / 2,
            y: rows[1].y + rows[1].height.saturating_sub(qr_height) / 2,
            width: qr_width,
            height: qr_height,
        };
        frame.render_widget(
            Paragraph::new(symbol.as_str())
                .style(Style::default().fg(Color::Black).bg(Color::White)),
            qr_area,
        );
    } else {
        frame.render_widget(
            Paragraph::new("Terminal is too small to display a scannable QR code")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Yellow)),
            rows[1],
        );
    }

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "P",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Phone login  "),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Cancel"),
        ])),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(format!(
            " waiting for scan · refreshes automatically · expires in {expires_in}s"
        ))
        .style(Style::default().fg(Color::Black).bg(Color::DarkGray)),
        rows[3],
    );
}

pub(crate) fn chord_from_crossterm(
    code: CrosstermKey,
    modifiers: KeyModifiers,
) -> Option<KeyChord> {
    let key = match code {
        CrosstermKey::Char(character) => Key::Char(character.to_ascii_lowercase()),
        CrosstermKey::Up => Key::Up,
        CrosstermKey::Down => Key::Down,
        CrosstermKey::Left => Key::Left,
        CrosstermKey::Right => Key::Right,
        CrosstermKey::Home => Key::Home,
        CrosstermKey::End => Key::End,
        CrosstermKey::Enter => Key::Enter,
        CrosstermKey::Esc => Key::Escape,
        _ => return None,
    };
    Some(KeyChord {
        key,
        control: modifiers.contains(KeyModifiers::CONTROL),
        shift: modifiers.contains(KeyModifiers::SHIFT),
        alt: modifiers.contains(KeyModifiers::ALT),
    })
}
use super::*;
