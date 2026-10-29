//! Mission colours, from the terminal palette to CSS.

use ratatui::style::Color;

/// Named terminal colours, tuned to read on the night globe.
fn named(c: Color) -> Option<&'static str> {
    Some(match c {
        Color::Black => "#64748b",
        Color::Red => "#ff6b6b",
        Color::Green => "#4ade80",
        Color::Yellow => "#facc15",
        Color::Blue => "#60a5fa",
        Color::Magenta => "#e879f9",
        Color::Cyan => "#22d3ee",
        Color::Gray => "#cbd5e1",
        Color::DarkGray => "#94a3b8",
        Color::LightRed => "#ff9a7a",
        Color::LightGreen => "#86efac",
        Color::LightYellow => "#fde68a",
        Color::LightBlue => "#93c5fd",
        Color::LightMagenta => "#f0abfc",
        Color::LightCyan => "#67e8f9",
        Color::White => "#f8fafc",
        _ => return None,
    })
}

const NAMED: [Color; 16] = [
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Gray,
    Color::DarkGray,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
    Color::White,
];

pub fn css(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(n) if n < 16 => css(NAMED[n as usize]),
        // The 6×6×6 colour cube of the 256-colour palette.
        Color::Indexed(n) if n < 232 => {
            let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            let i = n - 16;
            format!("#{:02x}{:02x}{:02x}", level(i / 36), level(i / 6 % 6), level(i % 6))
        }
        Color::Indexed(n) => {
            let v = 8 + 10 * (n - 232);
            format!("#{v:02x}{v:02x}{v:02x}")
        }
        other => named(other).unwrap_or("#e2e8f0").to_string(),
    }
}
