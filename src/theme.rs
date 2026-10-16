use anyhow::{bail, Context, Result};
use ratatui::style::Color;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub selection_bg: Color,
    pub cursor_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            selection_bg: Color::Rgb(64, 64, 64),
            cursor_bg: Color::Rgb(80, 80, 80),
        }
    }
}

impl Theme {
    pub fn from_env() -> Result<Self> {
        let defaults = Self::default();
        Ok(Self {
            selection_bg: env_color("GIT_DIFFTUI_SELECTION_BG", defaults.selection_bg)?,
            cursor_bg: env_color("GIT_DIFFTUI_CURSOR_BG", defaults.cursor_bg)?,
        })
    }
}

fn env_color(name: &str, fallback: Color) -> Result<Color> {
    match std::env::var(name) {
        Ok(value) => parse_color(&value).with_context(|| format!("Invalid {name}")),
        Err(std::env::VarError::NotPresent) => Ok(fallback),
        Err(error) => Err(error).with_context(|| format!("Invalid {name}")),
    }
}

fn parse_color(value: &str) -> Result<Color> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("expected a six-digit RGB color, e.g. #3A506B");
    }
    let rgb = u32::from_str_radix(hex, 16)?;
    Ok(Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rgb_colors() {
        assert_eq!(parse_color("#3A506B").unwrap(), Color::Rgb(58, 80, 107));
        assert_eq!(parse_color("707070").unwrap(), Color::Rgb(112, 112, 112));
        assert_eq!(parse_color("#aAbBcC").unwrap(), Color::Rgb(170, 187, 204));
    }

    #[test]
    fn rejects_invalid_colors() {
        for value in ["", "#abc", "#1234567", "red", "#GG0000", "é1234"] {
            assert!(parse_color(value).is_err(), "{value}");
        }
    }
}
