use ratatui::style::{Color, Modifier, Style};

pub const ACCENT: Color = Color::Rgb(255, 184, 72);
pub const BORDER: Color = Color::Rgb(58, 74, 96);
pub const MUTED: Color = Color::Rgb(122, 138, 156);
pub const TEXT: Color = Color::Rgb(226, 232, 240);
pub const PASS: Color = Color::Rgb(52, 211, 153);
pub const FAIL: Color = Color::Rgb(251, 113, 133);
pub const TLE: Color = Color::Rgb(251, 191, 36);
pub const INFO: Color = Color::Rgb(125, 211, 252);
pub const PANEL: Color = Color::Rgb(18, 24, 32);

pub fn title() -> Style {
    Style::default()
        .fg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

pub fn border() -> Style {
    Style::default().fg(BORDER)
}

pub fn border_active() -> Style {
    Style::default().fg(ACCENT)
}

pub fn muted() -> Style {
    Style::default().fg(MUTED)
}

pub fn text() -> Style {
    Style::default().fg(TEXT)
}

pub fn pass() -> Style {
    Style::default().fg(PASS).add_modifier(Modifier::BOLD)
}

pub fn fail() -> Style {
    Style::default().fg(FAIL).add_modifier(Modifier::BOLD)
}

pub fn tle() -> Style {
    Style::default().fg(TLE).add_modifier(Modifier::BOLD)
}

pub fn info() -> Style {
    Style::default().fg(INFO)
}
