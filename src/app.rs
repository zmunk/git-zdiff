use crate::{
    diff::{Diff, Kind},
    repo::{Operation, Snapshot},
};
use crossterm::event::KeyCode;
use ratatui::{
    prelude::*,
    widgets::{Paragraph, Wrap},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Line,
    Hunk,
}
#[derive(Clone, Debug)]
pub struct Nav {
    pub cursor: Option<usize>,
    pub mode: Mode,
    pub anchor: Option<usize>,
    pub scroll: usize,
}
impl Default for Nav {
    fn default() -> Self {
        Self {
            cursor: None,
            mode: Mode::Line,
            anchor: None,
            scroll: 0,
        }
    }
}
#[derive(Clone)]
pub struct Request {
    pub operation: Operation,
    pub selected: Vec<usize>,
    pub snapshot: Snapshot,
}
pub enum Intent {
    None,
    Quit,
    Mutate(Request),
}
pub struct App {
    pub snapshot: Snapshot,
    pub diffs: [Diff; 2],
    pub nav: [Nav; 2],
    pub tab: usize,
    pub preview_scroll: usize,
    pub horizontal: u16,
    pub message: String,
    pub valid: bool,
    pub confirmation: Option<Request>,
}
impl App {
    pub fn new(snapshot: Snapshot) -> Self {
        let diffs = [
            Diff::between(&snapshot.index, &snapshot.work),
            Diff::between(&snapshot.head, &snapshot.index),
        ];
        let nav = std::array::from_fn(|i| Nav {
            cursor: diffs[i].first(),
            ..Nav::default()
        });
        Self {
            snapshot,
            diffs,
            nav,
            tab: 0,
            preview_scroll: 0,
            horizontal: 0,
            message: String::new(),
            valid: true,
            confirmation: None,
        }
    }
    pub fn selecting(&self) -> bool {
        self.tab < 2 && self.nav[self.tab].anchor.is_some()
    }
    pub fn frozen(&self) -> bool {
        self.selecting() || self.confirmation.is_some()
    }
    pub fn replace(&mut self, snapshot: Snapshot) {
        let diffs = [
            Diff::between(&snapshot.index, &snapshot.work),
            Diff::between(&snapshot.head, &snapshot.index),
        ];
        for (i, diff) in diffs.iter().enumerate() {
            self.nav[i].cursor = diff.retain_cursor(&self.diffs[i], self.nav[i].cursor);
        }
        self.snapshot = snapshot;
        self.diffs = diffs;
        self.valid = true;
    }
    pub fn cancel_selection(&mut self) {
        for nav in &mut self.nav {
            nav.anchor = None;
        }
        self.confirmation = None;
    }
    pub fn selected(&self) -> Vec<usize> {
        if self.tab == 2 {
            return Vec::new();
        }
        let nav = &self.nav[self.tab];
        let diff = &self.diffs[self.tab];
        let Some(cursor) = nav.cursor else {
            return Vec::new();
        };
        if let Some(anchor) = nav.anchor {
            (anchor.min(cursor)..=anchor.max(cursor))
                .filter(|i| diff.actionable(*i))
                .collect()
        } else if nav.mode == Mode::Hunk {
            diff.hunk(cursor)
                .map(|h| {
                    diff.hunks[h]
                        .clone()
                        .filter(|i| diff.actionable(*i))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            vec![cursor]
        }
    }
    pub fn key(&mut self, key: KeyCode) -> Intent {
        if key == KeyCode::Char('q') {
            return Intent::Quit;
        }
        if self.confirmation.is_some() {
            match key {
                KeyCode::Char('y') => return Intent::Mutate(self.confirmation.take().unwrap()),
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.confirmation = None;
                    self.message = "Discard cancelled".into();
                }
                _ => {}
            }
            return Intent::None;
        }
        match key {
            KeyCode::Char('[') if !self.selecting() => {
                self.tab = (self.tab + 2) % 3;
            }
            KeyCode::Char(']') if !self.selecting() => {
                self.tab = (self.tab + 1) % 3;
            }
            KeyCode::Left => {
                self.horizontal = self.horizontal.saturating_sub(4);
            }
            KeyCode::Right => {
                self.horizontal = self.horizontal.saturating_add(4);
            }
            KeyCode::Char('j') | KeyCode::Down if self.tab == 2 => {
                self.preview_scroll = (self.preview_scroll + 1)
                    .min(self.snapshot.index.lines().count().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up if self.tab == 2 => {
                self.preview_scroll = self.preview_scroll.saturating_sub(1);
            }
            _ if self.tab == 2 => {}
            KeyCode::Esc => {
                self.nav[self.tab].anchor = None;
            }
            KeyCode::Tab if !self.selecting() => {
                let nav = &mut self.nav[self.tab];
                nav.mode = if nav.mode == Mode::Line {
                    Mode::Hunk
                } else {
                    Mode::Line
                };
            }
            KeyCode::Char('v') if self.nav[self.tab].mode == Mode::Line && !self.selecting() => {
                self.nav[self.tab].anchor = self.nav[self.tab].cursor;
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_cursor(true),
            KeyCode::Char('k') | KeyCode::Up => self.move_cursor(false),
            KeyCode::Char(' ') | KeyCode::Char('d') if self.valid => {
                let selected = self.selected();
                if selected.is_empty() || (key == KeyCode::Char('d') && self.tab != 0) {
                    return Intent::None;
                }
                let operation = if key == KeyCode::Char('d') {
                    Operation::Discard
                } else if self.tab == 0 {
                    Operation::Stage
                } else {
                    Operation::Unstage
                };
                let request = Request {
                    operation,
                    selected,
                    snapshot: self.snapshot.clone(),
                };
                if key == KeyCode::Char('d') {
                    self.confirmation = Some(request);
                } else {
                    return Intent::Mutate(request);
                }
            }
            _ => {}
        }
        Intent::None
    }
    fn move_cursor(&mut self, down: bool) {
        let diff = &self.diffs[self.tab];
        let nav = &mut self.nav[self.tab];
        let Some(cursor) = nav.cursor else {
            return;
        };
        let next = if nav.mode == Mode::Hunk {
            diff.hunk(cursor).and_then(|h| {
                let h = if down {
                    h.checked_add(1)
                } else {
                    h.checked_sub(1)
                }?;
                diff.hunks.get(h)?.clone().find(|i| diff.actionable(*i))
            })
        } else if nav.anchor.is_some() {
            let i = if down {
                cursor.checked_add(1)
            } else {
                cursor.checked_sub(1)
            };
            i.filter(|i| diff.actionable(*i))
        } else if down {
            (cursor + 1..diff.rows.len()).find(|i| diff.actionable(*i))
        } else {
            (0..cursor).rev().find(|i| diff.actionable(*i))
        };
        if next.is_some() {
            nav.cursor = next;
        }
    }
    pub fn draw(&mut self, frame: &mut Frame, name: &str) {
        let area = frame.area();
        let status_len = if self.confirmation.is_some() {
            150
        } else {
            self.message.chars().count()
        };
        let status_rows = status_len.div_ceil(area.width.max(1) as usize).clamp(1, 6) as u16;
        let regions = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(2 + status_rows),
        ])
        .split(area);
        let titles = ["Working", "Staged", "Preview"];
        let mut tabs = Vec::new();
        for (i, title) in titles.iter().enumerate() {
            if i > 0 {
                tabs.push(Span::raw(" | "));
            }
            let style = if i == self.tab {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Gray)
                    .add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
            } else if self.selecting() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };
            tabs.push(Span::styled(*title, style));
        }
        frame.render_widget(Paragraph::new(Line::from(tabs)), regions[0]);
        let mode = if self.tab == 2 {
            "read only"
        } else if self.selecting() {
            "multi-line"
        } else if self.nav[self.tab].mode == Mode::Hunk {
            "hunk"
        } else {
            "single-line"
        };
        frame.render_widget(
            Paragraph::new(format!("{name} — {mode}")).style(Style::default().fg(Color::DarkGray)),
            regions[1],
        );
        let body = regions[2];
        if self.tab == 2 {
            let lines: Vec<_> = self.snapshot.index.split_inclusive('\n').collect();
            self.preview_scroll = self.preview_scroll.min(lines.len().saturating_sub(1));
            if !self.valid {
                frame.render_widget(
                    Paragraph::new("Index content unavailable: resolve the refresh error below."),
                    body,
                );
            } else if lines.is_empty() {
                frame.render_widget(Paragraph::new("Index is empty"), body);
            } else {
                for (screen, text) in lines
                    .iter()
                    .skip(self.preview_scroll)
                    .take(body.height as usize)
                    .enumerate()
                {
                    self.render_row(
                        frame,
                        body,
                        screen,
                        &format!(
                            "{:>5}   {}",
                            self.preview_scroll + screen + 1,
                            display_text(text)
                        ),
                        Style::default(),
                    );
                }
            }
        } else {
            let diff = &self.diffs[self.tab];
            let nav = &mut self.nav[self.tab];
            let hunk = nav
                .cursor
                .and_then(|i| diff.hunk(i))
                .map(|h| diff.hunks[h].clone());
            let focus = if nav.mode == Mode::Hunk {
                hunk.as_ref().map(|h| h.start)
            } else {
                nav.cursor
            };
            if let Some(cursor) = focus {
                if cursor < nav.scroll {
                    nav.scroll = cursor;
                }
                if cursor >= nav.scroll + body.height as usize {
                    nav.scroll = cursor.saturating_sub(body.height.saturating_sub(1) as usize);
                }
            }
            nav.scroll = nav
                .scroll
                .min(diff.rows.len().saturating_sub(body.height as usize));
            let scroll = nav.scroll;
            if diff.rows.is_empty() {
                frame.render_widget(Paragraph::new("Empty file — no content changes"), body);
            }
            for (screen, (i, row)) in diff
                .rows
                .iter()
                .enumerate()
                .skip(scroll)
                .take(body.height as usize)
                .enumerate()
            {
                let (mark, color) = match row.kind {
                    Kind::Context => (' ', None),
                    Kind::Add => ('+', Some(Color::Green)),
                    Kind::Remove => ('-', Some(Color::Red)),
                };
                let mut style = Style::default();
                if let Some(color) = color {
                    style = style.fg(color);
                }
                if nav.mode == Mode::Hunk && hunk.as_ref().is_some_and(|h| h.contains(&i)) {
                    style = style.bg(Color::Rgb(55, 55, 55));
                } else if nav.cursor == Some(i) {
                    style = style.bg(Color::Rgb(65, 65, 65));
                } else if nav.anchor.is_some_and(|a| {
                    nav.cursor
                        .is_some_and(|c| (a.min(c)..=a.max(c)).contains(&i))
                }) {
                    style = style.bg(Color::Rgb(35, 40, 50));
                }
                let number = if row.kind == Kind::Remove {
                    row.old + 1
                } else {
                    row.new + 1
                };
                let line = format!("{number:>5} {mark} {}", display_text(&row.text));
                let rect = Rect::new(body.x, body.y + screen as u16, body.width, 1);
                let content = Paragraph::new(line)
                    .style(style)
                    .scroll((0, self.horizontal));
                frame.render_widget(content, rect);
            }
        }
        let footer = if self.tab == 0 {
            "Space: Stage selection  d: Discard (confirm)"
        } else if self.tab == 1 {
            "Space: Unstage selection"
        } else {
            "Read only"
        };
        let mut lines = vec![
            Line::raw(footer),
            Line::raw(
                "j/k: move  [/]: tabs  Tab: line/hunk  v: select  Esc: cancel  ←/→: pan  q: quit",
            ),
        ];
        if let Some(request) = &self.confirmation {
            let scope = if self.nav[0].mode == Mode::Hunk {
                "hunk"
            } else if self.selecting() {
                "multi-line selection"
            } else {
                "line"
            };
            lines.push(Line::styled(
                format!(
                    "Discard {} changed row(s) in this {scope}? [y confirms / n or Esc cancels]",
                    request.selected.len()
                ),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            lines.push(Line::styled(
                &self.message,
                Style::default().fg(if self.valid {
                    Color::Yellow
                } else {
                    Color::Red
                }),
            ));
        }
        let status = lines.split_off(2);
        let footer = regions[3];
        frame.render_widget(
            Paragraph::new(lines),
            Rect::new(footer.x, footer.y, footer.width, footer.height.min(2)),
        );
        if footer.height > 2 {
            frame.render_widget(
                Paragraph::new(status).wrap(Wrap { trim: false }),
                Rect::new(footer.x, footer.y + 2, footer.width, footer.height - 2),
            );
        }
    }
    fn render_row(&self, frame: &mut Frame, body: Rect, row: usize, text: &str, style: Style) {
        frame.render_widget(
            Paragraph::new(text)
                .style(style)
                .scroll((0, self.horizontal)),
            Rect::new(body.x, body.y + row as u16, body.width, 1),
        );
    }
}
fn display_text(text: &str) -> String {
    let no_newline = !text.ends_with('\n');
    let text = text.strip_suffix('\n').unwrap_or(text);
    let text = text.strip_suffix('\r').unwrap_or(text);
    let mut result = String::new();
    for ch in text.chars() {
        match ch {
            '\t' => result.push_str("    "),
            c if c.is_control() => result.push('�'),
            c => result.push(c),
        }
    }
    if no_newline {
        result.push_str(" ⟪no newline⟫");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        App::new(Snapshot {
            head: "a\nb\nc\nd\ne\n".into(),
            index: "a\nb\nc\nd\ne\n".into(),
            work: "A\nB\nc\nD\ne\n".into(),
            head_oid: "h".into(),
            index_oid: "i".into(),
            mode: "100644".into(),
            dev: 0,
            ino: 0,
            permissions: 0,
        })
    }
    fn key(a: &mut App, c: char) {
        a.key(KeyCode::Char(c));
    }
    #[test]
    fn selection_is_bounded_and_blocks_switching() {
        let mut a = app();
        key(&mut a, 'v');
        for _ in 0..20 {
            key(&mut a, 'j');
        }
        let cursor = a.nav[0].cursor.unwrap();
        assert!(!a.diffs[0].actionable(cursor + 1));
        key(&mut a, ']');
        a.key(KeyCode::Tab);
        assert_eq!(a.tab, 0);
        assert_eq!(a.nav[0].mode, Mode::Line);
        assert_eq!(a.selected(), (0..=cursor).collect::<Vec<_>>());
        a.key(KeyCode::Esc);
        key(&mut a, 'j');
        assert!(a.nav[0].cursor.unwrap() > cursor + 1);
    }
    #[test]
    fn tab_local_state_hunks_and_preview() {
        let mut a = app();
        key(&mut a, 'j');
        let cursor = a.nav[0].cursor;
        a.key(KeyCode::Tab);
        assert_eq!(a.nav[0].mode, Mode::Hunk);
        assert!(a.selected().len() > 1);
        key(&mut a, ']');
        assert_eq!(a.nav[1].mode, Mode::Line);
        assert_eq!(a.nav[1].cursor, None);
        key(&mut a, ']');
        assert!(matches!(a.key(KeyCode::Char(' ')), Intent::None));
        key(&mut a, ']');
        assert_eq!(a.nav[0].cursor, cursor);
        assert_eq!(a.nav[0].mode, Mode::Hunk);
    }
    #[test]
    fn confirmation_is_explicit_and_captures_snapshot() {
        let mut a = app();
        key(&mut a, 'd');
        assert!(a.confirmation.is_some());
        assert!(matches!(a.key(KeyCode::Enter), Intent::None));
        a.key(KeyCode::Esc);
        assert!(a.confirmation.is_none());
        key(&mut a, 'd');
        assert!(matches!(a.key(KeyCode::Char('y')), Intent::Mutate(_)));
    }
    #[test]
    fn logical_cursor_and_empty_state() {
        let mut a = app();
        let target = a.diffs[0]
            .rows
            .iter()
            .position(|r| r.text == "D\n")
            .unwrap();
        a.nav[0].cursor = Some(target);
        let mut snapshot = a.snapshot.clone();
        snapshot.work = format!("intro\n{}", snapshot.work);
        a.replace(snapshot);
        assert_eq!(a.diffs[0].rows[a.nav[0].cursor.unwrap()].text, "D\n");
        let mut snapshot = a.snapshot.clone();
        snapshot.work = snapshot.index.clone();
        a.replace(snapshot);
        assert_eq!(a.nav[0].cursor, None);
        assert!(a.selected().is_empty());
    }
}
