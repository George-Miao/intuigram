use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::{Image, Placement};

pub(crate) fn encode(placement: &Placement) -> Vec<u8> {
    let image = &placement.image;
    let target_width = u32::from(placement.size.columns)
        .saturating_mul(u32::from(placement.cell_pixels.width))
        .max(1);
    let target_height = u32::from(placement.size.rows)
        .saturating_mul(u32::from(placement.cell_pixels.height))
        .max(1);
    let mut colors = BTreeMap::<u8, (u8, u8, u8)>::new();
    for pixel in image.rgba().chunks_exact(4) {
        if pixel[3] != 0 {
            colors
                .entry(color_id(pixel))
                .or_insert_with(|| rgb332(pixel));
        }
    }

    let mut output = format!(
        "\x1b7\x1b[{};{}H\x1bPq\"1;1;{};{}",
        placement.y.saturating_add(1),
        placement.x.saturating_add(1),
        target_width,
        target_height
    );
    for (&id, &(red, green, blue)) in &colors {
        write!(
            output,
            "#{id};2;{};{};{}",
            percent(red),
            percent(green),
            percent(blue)
        )
        .expect("writing a Sixel palette to memory cannot fail");
    }
    let bands = target_height.div_ceil(6);
    for band in 0..bands {
        for (color_index, &id) in colors.keys().enumerate() {
            if color_index != 0 {
                output.push('$');
            }
            write!(output, "#{id}").expect("writing a Sixel color to memory cannot fail");
            encode_row(&mut output, image, target_width, target_height, band, id);
        }
        if band + 1 < bands {
            output.push('-');
        }
    }
    output.push_str("\x1b\\\x1b8");
    crate::kitty::wrap(output.into_bytes(), placement.multiplexer)
}

fn encode_row(
    output: &mut String,
    image: &Image,
    target_width: u32,
    target_height: u32,
    band: u32,
    id: u8,
) {
    let mut run = None::<(u8, u32)>;
    for x in 0..target_width {
        let value = sixel_value(image, target_width, target_height, x, band, id);
        match run {
            Some((current, count)) if current == value => run = Some((current, count + 1)),
            Some((current, count)) => {
                write_run(output, current, count);
                run = Some((value, 1));
            }
            None => run = Some((value, 1)),
        }
    }
    if let Some((value, count)) = run {
        write_run(output, value, count);
    }
}

fn sixel_value(
    image: &Image,
    target_width: u32,
    target_height: u32,
    x: u32,
    band: u32,
    id: u8,
) -> u8 {
    let mut bits = 0_u8;
    for bit in 0..6 {
        let y = band * 6 + bit;
        if y < target_height {
            let source_x = x
                .saturating_mul(image.width())
                .checked_div(target_width)
                .unwrap_or_default()
                .min(image.width().saturating_sub(1));
            let source_y = y
                .saturating_mul(image.height())
                .checked_div(target_height)
                .unwrap_or_default()
                .min(image.height().saturating_sub(1));
            let pixel = pixel(image, source_x, source_y);
            if pixel[3] != 0 && color_id(pixel) == id {
                bits |= 1 << bit;
            }
        }
    }
    63 + bits
}

fn write_run(output: &mut String, value: u8, count: u32) {
    if count >= 4 {
        write!(output, "!{count}{}", char::from(value))
            .expect("writing a Sixel run to memory cannot fail");
    } else {
        output.extend(std::iter::repeat_n(char::from(value), count as usize));
    }
}

fn pixel(image: &Image, x: u32, y: u32) -> &[u8] {
    let offset = ((y as usize) * (image.width() as usize) + (x as usize)) * 4;
    &image.rgba()[offset..offset + 4]
}

const fn color_id(pixel: &[u8]) -> u8 {
    (pixel[0] & 0xe0) | ((pixel[1] & 0xe0) >> 3) | (pixel[2] >> 6)
}

const fn rgb332(pixel: &[u8]) -> (u8, u8, u8) {
    (pixel[0] & 0xe0, pixel[1] & 0xe0, pixel[2] & 0xc0)
}

const fn percent(channel: u8) -> u16 {
    (channel as u16 * 100 + 127) / 255
}
