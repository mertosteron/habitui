use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap,
};
use ratatui::Frame;

use crate::config::Theme;
use crate::data::{Frequency, Habit, HabitKind, HabitStore};
use crate::tui::app::{
    AddForm, App, DetailState, EditForm, FormField, FrequencyChoice, KindChoice, Screen,
};

// Theme-independent accents.
const C_RED: Color = Color::Rgb(230, 95, 95);
const C_AMBER: Color = Color::Rgb(240, 190, 90);

// Streak milestone colors (theme-independent — same progression in every palette
// so the user gets consistent reward signals as their streak climbs).
const STREAK_WEEK: Color = Color::Rgb(120, 220, 240); // 7..=29
const STREAK_MONTH: Color = Color::Rgb(245, 200, 90); // 30..=99
const STREAK_QUARTER: Color = Color::Rgb(255, 140, 70); // 100..=364
const STREAK_YEAR: Color = Color::Rgb(255, 110, 200); // 365+

const PROGRESS_DAYS: i64 = 30;

const DOT_DONE: &str = "\u{25CF}";   // ●
const DOT_EMPTY: &str = "\u{25CB}";  // ○
const DOT_FAINT: &str = "\u{00B7}";  // ·

/// All theme-driven colors used by the renderer. Built once per frame from
/// `App::theme` and passed to helpers — the rest of the file is theme-agnostic.
#[derive(Clone, Copy)]
pub struct Palette {
    pub primary: Color,
    pub primary_dim: Color,
    pub primary_dark: Color,
    pub primary_faint: Color,
    pub primary_bg: Color,
    pub text: Color,
    pub text_dim: Color,
    /// 4-level activity ramp for the global heatmap (lightest to brightest).
    pub level_1: Color,
    pub level_2: Color,
    pub level_3: Color,
    pub level_4: Color,
    /// Color for failure markers on Quit habits. Picked per-theme so it does
    /// not collide with the primary palette (e.g. on Red theme, fails would
    /// otherwise look identical to streak/clean days).
    pub fail: Color,
}

pub fn palette_for(theme: Theme) -> Palette {
    match theme {
        Theme::Green => Palette {
            primary: Color::Rgb(80, 240, 130),
            primary_dim: Color::Rgb(50, 160, 90),
            primary_dark: Color::Rgb(30, 90, 55),
            primary_faint: Color::Rgb(20, 55, 35),
            primary_bg: Color::Rgb(12, 38, 24),
            text: Color::Rgb(150, 230, 170),
            text_dim: Color::Rgb(80, 130, 95),
            level_1: Color::Rgb(40, 110, 70),
            level_2: Color::Rgb(60, 170, 100),
            level_3: Color::Rgb(90, 220, 130),
            level_4: Color::Rgb(140, 255, 170),
            fail: Color::Rgb(230, 95, 95),
        },
        Theme::Blue => Palette {
            primary: Color::Rgb(90, 175, 250),
            primary_dim: Color::Rgb(60, 120, 190),
            primary_dark: Color::Rgb(35, 75, 120),
            primary_faint: Color::Rgb(20, 45, 70),
            primary_bg: Color::Rgb(12, 28, 50),
            text: Color::Rgb(170, 205, 240),
            text_dim: Color::Rgb(90, 120, 160),
            level_1: Color::Rgb(45, 90, 140),
            level_2: Color::Rgb(70, 140, 200),
            level_3: Color::Rgb(110, 185, 240),
            level_4: Color::Rgb(170, 225, 255),
            fail: Color::Rgb(230, 95, 95),
        },
        Theme::Red => Palette {
            primary: Color::Rgb(245, 110, 110),
            primary_dim: Color::Rgb(180, 75, 75),
            primary_dark: Color::Rgb(110, 45, 45),
            primary_faint: Color::Rgb(60, 25, 25),
            primary_bg: Color::Rgb(45, 18, 18),
            text: Color::Rgb(245, 185, 180),
            text_dim: Color::Rgb(150, 100, 100),
            level_1: Color::Rgb(130, 55, 55),
            level_2: Color::Rgb(195, 90, 90),
            level_3: Color::Rgb(240, 130, 120),
            level_4: Color::Rgb(255, 180, 165),
            // Magenta — distinct from every red in the rest of this palette.
            fail: Color::Rgb(225, 80, 200),
        },
        Theme::Yellow => Palette {
            primary: Color::Rgb(245, 215, 90),
            primary_dim: Color::Rgb(185, 160, 60),
            primary_dark: Color::Rgb(115, 95, 35),
            primary_faint: Color::Rgb(65, 55, 22),
            primary_bg: Color::Rgb(45, 38, 16),
            text: Color::Rgb(235, 220, 175),
            text_dim: Color::Rgb(150, 135, 90),
            level_1: Color::Rgb(135, 110, 40),
            level_2: Color::Rgb(195, 165, 65),
            level_3: Color::Rgb(240, 210, 100),
            level_4: Color::Rgb(255, 235, 165),
            fail: Color::Rgb(230, 95, 95),
        },
    }
}

/// Map a streak length (calendar days) to its milestone color. The progression
/// is universal across themes so the visual reward is consistent.
fn streak_color(streak: u32, palette: &Palette) -> Color {
    match streak {
        0 => palette.text_dim,
        1..=6 => palette.primary,
        7..=29 => STREAK_WEEK,
        30..=99 => STREAK_MONTH,
        100..=364 => STREAK_QUARTER,
        _ => STREAK_YEAR,
    }
}

pub fn render(f: &mut Frame, app: &mut App) {
    let palette = palette_for(app.theme);
    match &app.screen {
        Screen::List => render_list(f, &palette, app),
        Screen::AddHabit(_) => {
            render_list(f, &palette, app);
            if let Screen::AddHabit(form) = &app.screen {
                render_add_form(f, &palette, form);
            }
        }
        Screen::EditHabit(_) => {
            render_list(f, &palette, app);
            if let Screen::EditHabit(form) = &app.screen {
                render_edit_form(f, &palette, form);
            }
        }
        Screen::Detail(state) => {
            render_detail(f, &palette, app, state);
        }
        Screen::GlobalHeatmap => render_global_heatmap(f, &palette, app),
        Screen::ConfirmDelete { habit_id } => {
            let id = *habit_id;
            render_list(f, &palette, app);
            render_confirm_delete(f, &palette, app, id);
        }
        Screen::ConfirmPastEdit(state) => {
            render_detail(f, &palette, app, state);
            render_confirm_past_edit(f, &palette, app, state);
        }
    }
}

pub fn render_resize_notice(f: &mut Frame, area: Rect) {
    // Theme-independent: shown when the window is too small.
    let msg = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "Please resize to at least 100x24",
            Style::default().fg(C_AMBER).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "(Press Ctrl-C to quit.)",
            Style::default().fg(Color::Rgb(120, 120, 120)),
        )),
    ])
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true });
    f.render_widget(msg, area);
}

// ---------- List screen ----------

fn render_list(f: &mut Frame, p: &Palette, app: &mut App) {
    let outer = f.area();

    // The list row needs ~95 cells (gutter + name + 30 spaced circles + streak)
    // so the side gutter has to stay small at the minimum width. Scale up only
    // on much wider terminals where the extra breathing room actually fits.
    let h_pad: u16 = if outer.width >= 132 { (outer.width / 32).max(2) } else { 2 };
    let inner_w = outer.width.saturating_sub(h_pad * 2);
    let inner = Rect::new(outer.x + h_pad, outer.y + 1, inner_w, outer.height.saturating_sub(2));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "HABITUI",
            Style::default().fg(p.primary).add_modifier(Modifier::BOLD),
        ))),
        chunks[0],
    );

    let date_line = format_long_date(app.today).to_uppercase();
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            date_line,
            Style::default().fg(p.primary_dim),
        ))),
        chunks[1],
    );

    render_habit_table(f, p, app, chunks[3], chunks[5]);

    let status_line: Line = match &app.status {
        Some(msg) => Line::from(Span::styled(msg.clone(), Style::default().fg(C_AMBER))),
        None => Line::from(""),
    };
    f.render_widget(Paragraph::new(status_line), chunks[6]);

    f.render_widget(render_status_bar(p, app), chunks[7]);
}

fn render_habit_table(f: &mut Frame, p: &Palette, app: &mut App, header_area: Rect, body_area: Rect) {
    let name_w: u16 = 22;
    // Each day takes 2 cells (circle + trailing space) so glyphs read as
    // individual, spaced-out beads. PROGRESS_DAYS * 2 covers them all.
    let progress_w: u16 = (PROGRESS_DAYS as u16) * 2;
    let streak_w: u16 = 10;

    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(name_w),
            Constraint::Length(progress_w),
            Constraint::Min(streak_w),
        ])
        .split(header_area);

    let header_style = Style::default().fg(p.text_dim).add_modifier(Modifier::BOLD);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("HABIT", header_style))),
        header_layout[1],
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("30-DAY PROGRESS", header_style))),
        header_layout[2],
    );
    f.render_widget(
        Paragraph::new(
            Line::from(Span::styled("STREAK", header_style)).alignment(Alignment::Right),
        ),
        header_layout[3],
    );

    if app.store.habits.is_empty() {
        let empty = Paragraph::new(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                "(no habits yet — press [a] to add one)",
                Style::default().fg(p.text_dim),
            ),
        ]));
        f.render_widget(empty, body_area);
        return;
    }

    let rows: Vec<Row> = app
        .store
        .habits
        .iter()
        .map(|h| habit_row(p, h, app.today, name_w as usize, PROGRESS_DAYS as usize))
        .collect();

    let widths = [
        Constraint::Length(name_w),
        Constraint::Length(progress_w),
        Constraint::Min(streak_w),
    ];
    let table = Table::new(rows, widths)
        .column_spacing(0)
        .highlight_style(Style::default().bg(p.primary_bg).add_modifier(Modifier::BOLD))
        .highlight_symbol(Span::styled(
            "▶  ",
            Style::default().fg(p.primary).add_modifier(Modifier::BOLD),
        ));

    let mut state = TableState::default();
    state.select(Some(app.selected));
    f.render_stateful_widget(table, body_area, &mut state);
}

fn habit_row(p: &Palette, h: &Habit, today: NaiveDate, name_w: usize, days: usize) -> Row<'static> {
    let is_quit = matches!(h.kind, HabitKind::Quit { .. });

    let (dot, dot_color) = if is_quit {
        ("◆", p.primary_dim)
    } else {
        ("●", p.primary)
    };

    let name = truncate(&h.name, name_w.saturating_sub(3));

    let name_line = Line::from(vec![
        Span::styled(dot, Style::default().fg(dot_color)),
        Span::raw(" "),
        Span::styled(name, Style::default().fg(p.text).add_modifier(Modifier::BOLD)),
    ]);

    let progress_line = Line::from(progress_bar_spans(p, h, today, days));
    let streak_line = streak_line(p, h, today, is_quit);

    Row::new(vec![
        Cell::from(name_line),
        Cell::from(progress_line),
        Cell::from(streak_line.alignment(Alignment::Right)),
    ])
    .height(1)
}

/// Build the 30-day progress strip as individual, spaced-out circle glyphs:
/// `●` for completed/clean, `○` for missed, `·` for pre-creation days.
/// Quit habits surface failure days as a red `●` so they stand out. Each day
/// is followed by a single space to give the row a beaded-strand look.
fn progress_bar_spans(p: &Palette, h: &Habit, today: NaiveDate, days: usize) -> Vec<Span<'static>> {
    let start = today - Duration::days(days as i64 - 1);
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(days * 2);
    let mut d = start;
    for i in 0..days {
        let span = if d < h.created_at {
            Span::styled(DOT_FAINT, Style::default().fg(p.primary_faint))
        } else {
            match &h.kind {
                HabitKind::Build => {
                    if h.completions.contains(&d) {
                        Span::styled(DOT_DONE, Style::default().fg(p.primary).add_modifier(Modifier::BOLD))
                    } else {
                        Span::styled(DOT_EMPTY, Style::default().fg(p.primary_dark))
                    }
                }
                HabitKind::Quit { failures } => {
                    if failures.contains(&d) {
                        Span::styled(DOT_DONE, Style::default().fg(p.fail).add_modifier(Modifier::BOLD))
                    } else {
                        Span::styled(DOT_DONE, Style::default().fg(p.primary).add_modifier(Modifier::BOLD))
                    }
                }
            }
        };
        spans.push(span);
        if i + 1 < days {
            spans.push(Span::raw(" "));
        }
        d += Duration::days(1);
    }
    spans
}

fn streak_line(p: &Palette, h: &Habit, today: NaiveDate, is_quit: bool) -> Line<'static> {
    let streak = h.current_streak(today);
    if streak == 0 {
        return Line::from(vec![Span::styled("—", Style::default().fg(p.text_dim))]);
    }
    let color = streak_color(streak, p);
    let suffix = if is_quit { " 🌿" } else { " 🔥" };
    Line::from(vec![
        Span::styled(
            format!("{}", streak),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(suffix, Style::default()),
    ])
}

fn render_status_bar<'a>(p: &Palette, app: &App) -> Paragraph<'a> {
    let scheduled_today = app
        .store
        .habits
        .iter()
        .filter(|h| {
            matches!(h.kind, HabitKind::Build)
                && (h.is_due(app.today) || h.completions.contains(&app.today))
        })
        .count();
    let done_today = app
        .store
        .habits
        .iter()
        .filter(|h| matches!(h.kind, HabitKind::Build) && h.completions.contains(&app.today))
        .count();

    let current_is_quit = app
        .store
        .habits
        .get(app.selected)
        .map(|h| matches!(h.kind, HabitKind::Quit { .. }))
        .unwrap_or(false);

    let sep = "    ";

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(
        "HABITUI v1.0",
        Style::default().fg(p.primary).add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(sep, Style::default().fg(p.primary_dark)));
    spans.push(Span::styled(format!("{}", app.today), Style::default().fg(p.primary_dim)));
    spans.push(Span::styled(sep, Style::default().fg(p.primary_dark)));
    spans.push(Span::styled(
        format!("{}/{} today", done_today, scheduled_today),
        Style::default().fg(p.primary_dim),
    ));
    spans.push(Span::styled(sep, Style::default().fg(p.primary_dark)));
    spans.push(Span::styled(
        format!("theme: {}", app.theme.label()),
        Style::default().fg(p.text_dim),
    ));
    spans.push(Span::styled(sep, Style::default().fg(p.primary_dark)));
    extend_spans(&mut spans, key_hint(p, "↑↓/jk", "NAV"));
    spans.push(Span::raw(" "));
    if current_is_quit {
        extend_spans(&mut spans, key_hint(p, "F", "FAIL"));
    } else {
        extend_spans(&mut spans, key_hint(p, "SPACE", "CHECK"));
    }
    spans.push(Span::raw(" "));
    extend_spans(&mut spans, key_hint(p, "ENTER", "GRAPH"));
    spans.push(Span::raw(" "));
    extend_spans(&mut spans, key_hint(p, "A", "ADD"));
    spans.push(Span::raw(" "));
    extend_spans(&mut spans, key_hint(p, "E", "EDIT"));
    spans.push(Span::raw(" "));
    extend_spans(&mut spans, key_hint(p, "D", "DEL"));
    spans.push(Span::raw(" "));
    extend_spans(&mut spans, key_hint(p, "G", "GLOBAL"));
    spans.push(Span::raw(" "));
    extend_spans(&mut spans, key_hint(p, "C", "THEME"));

    Paragraph::new(Line::from(spans))
}

fn key_hint<'a>(p: &Palette, key: &str, label: &str) -> Vec<Span<'a>> {
    vec![
        Span::styled(
            key.to_string(),
            Style::default().fg(p.primary).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(label.to_string(), Style::default().fg(p.text_dim)),
    ]
}

fn extend_spans<'a>(out: &mut Vec<Span<'a>>, parts: Vec<Span<'a>>) {
    out.extend(parts);
}

// ---------- helpers ----------

fn truncate(s: &str, width: usize) -> String {
    let mut out = String::new();
    let mut len = 0;
    for c in s.chars() {
        if len + 1 > width {
            break;
        }
        out.push(c);
        len += 1;
    }
    out
}

fn format_long_date(d: NaiveDate) -> String {
    d.format("%A, %B %-d, %Y").to_string()
}

fn format_frequency(f: Frequency) -> String {
    match f {
        Frequency::Daily => "Daily".to_string(),
        Frequency::EveryNDays(n) => format!("Every {} days", n),
        Frequency::NTimesPerWeek(n) => format!("{}× per week", n),
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

// ---------- Add form ----------

fn render_add_form(f: &mut Frame, p: &Palette, form: &AddForm) {
    let is_quit = matches!(form.kind_choice, KindChoice::Quit);
    let height = if is_quit {
        12
    } else if form.freq_choice.needs_numeric() {
        17
    } else {
        15
    };
    let area = centered_rect(64, height, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.primary))
        .title(Span::styled(
            " NEW HABIT ",
            Style::default().fg(p.primary).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if is_quit {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(inner);

        f.render_widget(field_label(p, "Name", form.field == FormField::Name), layout[0]);
        f.render_widget(name_paragraph(p, &form.name, form.field == FormField::Name), layout[1]);
        f.render_widget(field_label(p, "Type", form.field == FormField::Kind), layout[3]);
        f.render_widget(kind_picker(p, form.kind_choice), layout[4]);
        f.render_widget(quit_implicit_daily_note(p), layout[6]);
        f.render_widget(form_help(p, form.error.as_deref(), "Tab to switch · Enter to save · Esc to cancel"), layout[8]);
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

    f.render_widget(field_label(p, "Name", form.field == FormField::Name), layout[0]);
    f.render_widget(name_paragraph(p, &form.name, form.field == FormField::Name), layout[1]);

    f.render_widget(field_label(p, "Type", form.field == FormField::Kind), layout[3]);
    f.render_widget(kind_picker(p, form.kind_choice), layout[4]);

    f.render_widget(field_label(p, "Frequency", form.field == FormField::Frequency), layout[6]);
    f.render_widget(freq_picker(p, form.freq_choice), layout[7]);

    if form.freq_choice.needs_numeric() {
        f.render_widget(
            numeric_field(p, form.freq_choice, &form.numeric_buf, form.field == FormField::NumericValue),
            layout[8],
        );
    }

    f.render_widget(form_help(p, form.error.as_deref(), "Tab to switch · Enter to save · Esc to cancel"), layout[10]);
}

fn quit_implicit_daily_note<'a>(p: &Palette) -> Paragraph<'a> {
    Paragraph::new(Line::from(vec![
        Span::styled("  Frequency: ", Style::default().fg(p.text_dim)),
        Span::styled(
            "Daily (implicit for Quit habits)",
            Style::default().fg(p.text).add_modifier(Modifier::ITALIC),
        ),
    ]))
}

// ---------- Edit form ----------

fn render_edit_form(f: &mut Frame, p: &Palette, form: &EditForm) {
    let height = if form.is_quit {
        12
    } else if form.freq_choice.needs_numeric() {
        17
    } else {
        15
    };
    let area = centered_rect(64, height, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(C_AMBER))
        .title(Span::styled(
            " EDIT HABIT ",
            Style::default().fg(C_AMBER).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if form.is_quit {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(inner);

        f.render_widget(field_label(p, "Name", form.field == FormField::Name), layout[0]);
        f.render_widget(name_paragraph(p, &form.name, form.field == FormField::Name), layout[1]);
        f.render_widget(field_label(p, "Type (read-only)", false), layout[3]);
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    form.kind_label,
                    Style::default().fg(p.text).add_modifier(Modifier::ITALIC),
                ),
            ])),
            layout[4],
        );
        f.render_widget(quit_implicit_daily_note(p), layout[6]);
        f.render_widget(form_help(p, form.error.as_deref(), "Enter to save · Esc to cancel"), layout[8]);
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

    f.render_widget(field_label(p, "Name", form.field == FormField::Name), layout[0]);
    f.render_widget(name_paragraph(p, &form.name, form.field == FormField::Name), layout[1]);

    f.render_widget(field_label(p, "Type (read-only)", false), layout[3]);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                form.kind_label,
                Style::default().fg(p.text).add_modifier(Modifier::ITALIC),
            ),
        ])),
        layout[4],
    );

    f.render_widget(field_label(p, "Frequency", form.field == FormField::Frequency), layout[6]);
    f.render_widget(freq_picker(p, form.freq_choice), layout[7]);

    if form.freq_choice.needs_numeric() {
        f.render_widget(
            numeric_field(p, form.freq_choice, &form.numeric_buf, form.field == FormField::NumericValue),
            layout[8],
        );
    }

    f.render_widget(form_help(p, form.error.as_deref(), "Tab to switch · Enter to save · Esc to cancel"), layout[10]);
}

fn field_label<'a>(p: &Palette, text: &'a str, focused: bool) -> Paragraph<'a> {
    let style = if focused {
        Style::default().fg(p.primary).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(p.text_dim)
    };
    Paragraph::new(Span::styled(text, style))
}

fn name_paragraph<'a>(p: &Palette, name: &'a str, focused: bool) -> Paragraph<'a> {
    let display = if name.is_empty() {
        Span::styled(" <type a name>", Style::default().fg(p.text_dim))
    } else {
        Span::styled(format!(" {}", name), Style::default().fg(p.text))
    };
    let style = if focused {
        Style::default().bg(p.primary_bg)
    } else {
        Style::default()
    };
    Paragraph::new(Line::from(vec![display])).style(style)
}

fn kind_picker<'a>(p: &Palette, choice: KindChoice) -> Paragraph<'a> {
    let mut spans: Vec<Span> = Vec::new();
    for (c, label) in [(KindChoice::Build, "[Build]"), (KindChoice::Quit, "[Quit]")] {
        let selected = c == choice;
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(p.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.text_dim)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    Paragraph::new(Line::from(spans))
}

fn freq_picker<'a>(p: &Palette, choice: FrequencyChoice) -> Paragraph<'a> {
    let options = [
        (FrequencyChoice::Daily, "[Daily]"),
        (FrequencyChoice::EveryNDays, "[Every N days]"),
        (FrequencyChoice::NTimesPerWeek, "[N× / week]"),
    ];
    let mut spans: Vec<Span> = Vec::new();
    for (c, label) in options {
        let selected = c == choice;
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(p.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.text_dim)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    Paragraph::new(Line::from(spans))
}

fn numeric_field<'a>(p: &Palette, choice: FrequencyChoice, buf: &str, focused: bool) -> Paragraph<'a> {
    let label = match choice {
        FrequencyChoice::EveryNDays => "Days (N)",
        FrequencyChoice::NTimesPerWeek => "Times per week (N)",
        _ => "N",
    };
    let label_style = if focused {
        Style::default().fg(p.primary).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(p.text_dim)
    };
    let value_style = if focused {
        Style::default().bg(p.primary_bg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(p.text)
    };
    let value_display = if buf.is_empty() {
        Span::styled(" _ ", Style::default().fg(p.text_dim))
    } else {
        Span::styled(format!(" {} ", buf), value_style)
    };
    Paragraph::new(Line::from(vec![
        Span::styled(format!("{}: ", label), label_style),
        value_display,
        Span::styled(
            "  digits to type · backspace to clear",
            Style::default().fg(p.text_dim),
        ),
    ]))
}

fn form_help<'a>(p: &Palette, err: Option<&'a str>, fallback: &'a str) -> Paragraph<'a> {
    let line = match err {
        Some(e) => Line::from(Span::styled(
            e.to_string(),
            Style::default().fg(C_RED).add_modifier(Modifier::BOLD),
        )),
        None => Line::from(Span::styled(
            fallback.to_string(),
            Style::default().fg(p.text_dim),
        )),
    };
    Paragraph::new(line)
}

// ---------- Confirm-delete ----------

fn render_confirm_delete(f: &mut Frame, p: &Palette, app: &App, habit_id: u64) {
    let name = app
        .store
        .habit(habit_id)
        .map(|h| h.name.clone())
        .unwrap_or_else(|| "<missing>".to_string());

    let area = centered_rect(54, 6, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(C_RED))
        .title(Span::styled(
            " CONFIRM DELETE ",
            Style::default().fg(C_RED).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Delete \"", Style::default().fg(p.text)),
            Span::styled(name, Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
            Span::styled("\" ?", Style::default().fg(p.text)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Y] ", Style::default().fg(C_RED).add_modifier(Modifier::BOLD)),
            Span::styled("yes  ", Style::default().fg(p.text)),
            Span::styled("[N] ", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
            Span::styled("no", Style::default().fg(p.text)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
}

// ---------- Confirm past-day edit ----------

fn render_confirm_past_edit(f: &mut Frame, p: &Palette, app: &App, state: &DetailState) {
    let habit_name = app
        .store
        .habit(state.habit_id)
        .map(|h| h.name.clone())
        .unwrap_or_else(|| "<missing>".to_string());

    let area = centered_rect(60, 7, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(C_AMBER))
        .title(Span::styled(
            " CONFIRM PAST-DAY EDIT ",
            Style::default().fg(C_AMBER).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Edit \"", Style::default().fg(p.text)),
            Span::styled(habit_name, Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
            Span::styled("\" on ", Style::default().fg(p.text)),
            Span::styled(
                state.cursor.format("%a %Y-%m-%d").to_string(),
                Style::default().fg(C_AMBER).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ?", Style::default().fg(p.text)),
        ]),
        Line::from(Span::styled(
            "Changing a past day rewrites history.",
            Style::default().fg(p.text_dim),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Y] ", Style::default().fg(C_AMBER).add_modifier(Modifier::BOLD)),
            Span::styled("yes  ", Style::default().fg(p.text)),
            Span::styled("[N] ", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
            Span::styled("no", Style::default().fg(p.text)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
}

// ---------- Detail ----------

fn render_detail(f: &mut Frame, p: &Palette, app: &App, state: &DetailState) {
    let area = f.area();
    let habit = match app.store.habit(state.habit_id) {
        Some(h) => h,
        None => {
            f.render_widget(
                Paragraph::new(Span::styled(
                    "Habit not found. Press q to return.",
                    Style::default().fg(p.text),
                ))
                .alignment(Alignment::Center),
                area,
            );
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    render_detail_header(f, p, habit, app.today, chunks[0]);
    render_motivation_line(f, p, habit, app.today, chunks[1]);
    render_recent_strip(f, p, habit, app.today, app.year, chunks[2]);
    render_binary_calendar(f, p, habit, app.today, app.year, state, chunks[3]);

    let status_line = match &app.status {
        Some(msg) => Line::from(Span::styled(format!("  {}", msg), Style::default().fg(C_AMBER))),
        None => Line::from(""),
    };
    f.render_widget(Paragraph::new(status_line), chunks[4]);

    let footer = if state.edit_mode {
        Paragraph::new(Line::from(vec![
            Span::styled(
                "  EDIT ",
                Style::default()
                    .fg(Color::Black)
                    .bg(C_AMBER)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("←↑↓→ ", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
            Span::styled("MOVE", Style::default().fg(p.text_dim)),
            Span::styled("    ", Style::default()),
            Span::styled("SPACE ", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
            Span::styled("TOGGLE", Style::default().fg(p.text_dim)),
            Span::styled("    ", Style::default()),
            Span::styled("E/ESC ", Style::default().fg(C_RED).add_modifier(Modifier::BOLD)),
            Span::styled("EXIT EDIT", Style::default().fg(p.text_dim)),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled("  ESC/Q/ENTER ", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
            Span::styled("BACK", Style::default().fg(p.text_dim)),
            Span::styled("    ", Style::default()),
            Span::styled("E ", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
            Span::styled("EDIT PAST DAYS", Style::default().fg(p.text_dim)),
            Span::styled("    ", Style::default()),
            Span::styled("[ / ] ", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
            Span::styled("YEAR", Style::default().fg(p.text_dim)),
        ]))
    };
    f.render_widget(footer, chunks[5]);
}

/// Activity-by-day strip. Replaces the old aggregate Sparkline with a row of
/// individual circles (`●` hit / `○` miss) — same visual language as the
/// 30-day progress on the list screen. We show as many days as fit in the
/// available width, anchored on the right at `today` (or year-end if a past
/// year is selected).
fn render_recent_strip(f: &mut Frame, p: &Palette, habit: &Habit, today: NaiveDate, year: i32, area: Rect) {
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });
    let title = if is_quit {
        format!(" {} · FAILURES BY DAY ", year)
    } else {
        format!(" {} · ACTIVITY BY DAY ", year)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.primary_dark))
        .title(Span::styled(
            title,
            Style::default().fg(p.primary_dim).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let anchor = if year == today.year() {
        today
    } else {
        let ye = NaiveDate::from_ymd_opt(year, 12, 31).unwrap_or(today);
        if ye > today { today } else { ye }
    };

    // Same beaded-strand aesthetic as the 30-day strip: each day takes 2
    // cells (circle + trailing space). Compute how many fit, then anchor on
    // the right at `today` (or year-end for past years).
    let pad: u16 = 2;
    let usable_cells = inner.width.saturating_sub(pad) as i64;
    let days_to_show = (usable_cells / 2).max(0);
    if days_to_show == 0 {
        return;
    }
    let start = anchor - Duration::days(days_to_show - 1);

    let mut spans: Vec<Span<'static>> = Vec::with_capacity((days_to_show as usize) * 2 + 1);
    spans.push(Span::raw(" ".repeat(pad as usize)));
    let mut d = start;
    let mut idx: i64 = 0;
    while d <= anchor {
        spans.push(activity_circle(p, habit, d, is_quit));
        idx += 1;
        if idx < days_to_show {
            spans.push(Span::raw(" "));
        }
        d += Duration::days(1);
    }

    // Center-ish vertical placement inside the (1-3 row) inner area.
    let row = inner.y + inner.height / 2;
    let line_area = Rect::new(inner.x, row, inner.width, 1);
    f.render_widget(Paragraph::new(Line::from(spans)), line_area);
}

fn activity_circle(p: &Palette, habit: &Habit, date: NaiveDate, is_quit: bool) -> Span<'static> {
    if date < habit.created_at {
        return Span::styled(DOT_FAINT, Style::default().fg(p.primary_faint));
    }
    let hit = match &habit.kind {
        HabitKind::Build => habit.completions.contains(&date),
        HabitKind::Quit { failures } => failures.contains(&date),
    };
    if is_quit {
        if hit {
            Span::styled(DOT_DONE, Style::default().fg(p.fail).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(DOT_DONE, Style::default().fg(p.primary).add_modifier(Modifier::BOLD))
        }
    } else if hit {
        Span::styled(DOT_DONE, Style::default().fg(p.primary).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(DOT_EMPTY, Style::default().fg(p.primary_dark))
    }
}

fn render_binary_calendar(
    f: &mut Frame,
    p: &Palette,
    habit: &Habit,
    today: NaiveDate,
    year: i32,
    state: &DetailState,
    area: Rect,
) {
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });
    let weeks: i64 = 18;

    let title = if state.edit_mode {
        format!(
            " {} · BINARY VIEW · EDITING {} ",
            year,
            state.cursor.format("%a %Y-%m-%d")
        )
    } else if is_quit {
        format!(" {} · BINARY VIEW · FAILURES ", year)
    } else {
        format!(" {} · BINARY VIEW · DONE / MISSED ", year)
    };
    let border_color = if state.edit_mode { C_AMBER } else { p.primary_dark };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default().fg(p.primary_dim).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let year_end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap_or(today);
    let anchor = if year == today.year() { today } else { year_end.min(today) };
    let dow = anchor.weekday().num_days_from_monday() as i64;
    let week_end = anchor + Duration::days(6 - dow);
    let total_days = weeks * 7;
    let week_start = week_end - Duration::days(total_days - 1);

    let labels = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
    let mut lines: Vec<Line> = Vec::new();

    for row in 0..7 {
        let mut spans: Vec<Span> = Vec::with_capacity(weeks as usize + 2);
        spans.push(Span::styled(
            format!(" {}  ", labels[row]),
            Style::default().fg(p.text_dim),
        ));
        for col in 0..weeks as usize {
            let date = week_start + Duration::days((col as i64) * 7 + row as i64);
            spans.push(binary_cell(p, habit, date, today, year, state));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
        if row < 6 {
            lines.push(Line::from(""));
        }
    }

    lines.push(Line::from(""));
    lines.push(binary_legend(p, is_quit));

    f.render_widget(Paragraph::new(lines), inner);
}

fn binary_cell(
    p: &Palette,
    habit: &Habit,
    date: NaiveDate,
    today: NaiveDate,
    year: i32,
    state: &DetailState,
) -> Span<'static> {
    let is_cursor = state.edit_mode && date == state.cursor;
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });

    let cell = "\u{2588}\u{2588}";
    let cell_half = "\u{2592}\u{2592}";

    if date > today {
        if is_cursor {
            return Span::styled(
                "[]",
                Style::default().fg(C_AMBER).add_modifier(Modifier::BOLD),
            );
        }
        return Span::styled("  ", Style::default());
    }
    if date.year() != year {
        let s = Style::default().fg(p.primary_faint);
        let s = if is_cursor { s.add_modifier(Modifier::REVERSED) } else { s };
        return Span::styled(cell, s);
    }
    if date < habit.created_at {
        let s = Style::default().fg(p.primary_faint);
        let s = if is_cursor { s.add_modifier(Modifier::REVERSED) } else { s };
        return Span::styled(cell, s);
    }

    let hit = match &habit.kind {
        HabitKind::Build => habit.completions.contains(&date),
        HabitKind::Quit { failures } => failures.contains(&date),
    };

    let (glyph, color) = if is_quit {
        if hit {
            (cell_half, p.fail)
        } else {
            (cell, p.primary_dim)
        }
    } else if hit {
        (cell, p.primary)
    } else {
        (cell, p.primary_dark)
    };

    let mut style = Style::default().fg(color);
    if is_cursor {
        style = style
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::REVERSED);
    }
    Span::styled(glyph, style)
}

fn binary_legend(p: &Palette, is_quit: bool) -> Line<'static> {
    let cell = "\u{2588}\u{2588}";
    let cell_half = "\u{2592}\u{2592}";
    if is_quit {
        Line::from(vec![
            Span::raw("    "),
            Span::styled("Legend: ", Style::default().fg(p.text_dim)),
            Span::styled(cell, Style::default().fg(p.primary_dim)),
            Span::styled(" clean   ", Style::default().fg(p.text_dim)),
            Span::styled(cell_half, Style::default().fg(p.fail)),
            Span::styled(" failure   ", Style::default().fg(p.text_dim)),
            Span::styled(cell, Style::default().fg(p.primary_faint)),
            Span::styled(" before created", Style::default().fg(p.text_dim)),
        ])
    } else {
        Line::from(vec![
            Span::raw("    "),
            Span::styled("Legend: ", Style::default().fg(p.text_dim)),
            Span::styled(cell, Style::default().fg(p.primary)),
            Span::styled(" done   ", Style::default().fg(p.text_dim)),
            Span::styled(cell, Style::default().fg(p.primary_dark)),
            Span::styled(" missed   ", Style::default().fg(p.text_dim)),
            Span::styled(cell, Style::default().fg(p.primary_faint)),
            Span::styled(" before created", Style::default().fg(p.text_dim)),
        ])
    }
}

fn render_detail_header(f: &mut Frame, p: &Palette, habit: &Habit, today: NaiveDate, area: Rect) {
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });
    let current = habit.current_streak(today);
    let longest = habit.longest_streak();
    let total = match &habit.kind {
        HabitKind::Build => habit.completions.len(),
        HabitKind::Quit { failures } => failures.len(),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.primary_dark));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1)])
        .split(inner);

    let kind_tag = if is_quit {
        Span::styled(
            " QUIT ",
            Style::default()
                .fg(Color::Black)
                .bg(p.primary_dim)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " BUILD ",
            Style::default()
                .fg(Color::Black)
                .bg(p.primary)
                .add_modifier(Modifier::BOLD),
        )
    };

    let streak_label = if is_quit {
        format!("{} days clean", current)
    } else {
        format!("{} day streak", current)
    };

    let streak_clr = streak_color(current, p);
    let dim_sep = Span::styled("   ·   ", Style::default().fg(p.primary_dark));

    let line = Line::from(vec![
        kind_tag,
        Span::raw("  "),
        Span::styled(
            habit.name.clone(),
            Style::default().fg(p.primary).add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(format_frequency(habit.frequency), Style::default().fg(p.text_dim)),
        dim_sep.clone(),
        Span::styled(
            streak_label,
            Style::default().fg(streak_clr).add_modifier(Modifier::BOLD),
        ),
        dim_sep.clone(),
        Span::styled("longest ", Style::default().fg(p.text_dim)),
        Span::styled(
            format!("{}", longest),
            Style::default().fg(p.text).add_modifier(Modifier::BOLD),
        ),
        dim_sep,
        Span::styled(
            if is_quit { "failures " } else { "total " },
            Style::default().fg(p.text_dim),
        ),
        Span::styled(
            format!("{}", total),
            Style::default().fg(p.text).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line), layout[0]);
}

fn render_motivation_line(f: &mut Frame, p: &Palette, habit: &Habit, today: NaiveDate, area: Rect) {
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });
    let streak = habit.current_streak(today);
    let msg = affirmation(streak, is_quit);
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            msg,
            Style::default().fg(p.primary_dim).add_modifier(Modifier::ITALIC),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn affirmation(streak: u32, is_quit: bool) -> String {
    let msg = if is_quit {
        match streak {
            0 => "A reset, not a defeat — start fresh tomorrow.",
            1..=2 => "One day at a time. The hardest part is starting.",
            3..=6 => "A few days clean — the urge gets quieter from here.",
            7..=13 => "A full week clean. Real progress.",
            14..=29 => "Two weeks. The new normal is taking shape.",
            30..=59 => "A month clean. This is who you are now.",
            60..=89 => "Two months clean. The old craving rarely calls anymore.",
            90..=119 => "Three months. The version of you that struggled feels far away.",
            120..=179 => "Four months in — clarity has replaced the noise.",
            180..=269 => "Half a year clean. You have rewritten a part of yourself.",
            270..=364 => "Nine months. What was a battle is now a quiet boundary.",
            365..=729 => "One full year clean. This is the new floor, not the ceiling.",
            _ => "Years now. The struggle is a memory; the freedom is the life.",
        }
    } else {
        match streak {
            0 => "Today is a good day to begin.",
            1..=2 => "First step taken. Keep it small, keep it kind.",
            3..=6 => "Three days in — the rhythm is forming.",
            7..=13 => "A week deep. Consistency over intensity.",
            14..=29 => "Two weeks. This is becoming a habit, not a chore.",
            30..=59 => "A full month. You don't need motivation any more — you have momentum.",
            60..=89 => "Two months. Identity is shifting — you are someone who does this.",
            90..=119 => "Three months. The habit now carries you on the days you would have skipped.",
            120..=179 => "Four months. Compounding, quietly, every single day.",
            180..=269 => "Half a year. Look back at who started — they would not believe this.",
            270..=364 => "Nine months. This is craft now, not effort.",
            365..=729 => "A full year. Streaks like this rewrite a life.",
            _ => "Years deep. This is mastery, quietly accumulating.",
        }
    };
    msg.to_string()
}

// ---------- Global heatmap ----------

fn render_global_heatmap(f: &mut Frame, p: &Palette, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(area);

    render_global_header(f, p, &app.store, app.today, app.year, chunks[0]);
    render_global_summary(f, p, &app.store, app.today, chunks[1]);
    render_global_grid(f, p, &app.store, app.today, app.year, chunks[2]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("  ESC/Q/G/ENTER ", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
        Span::styled("BACK TO LIST", Style::default().fg(p.text_dim)),
        Span::styled("    ", Style::default()),
        Span::styled("[ / ] ", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
        Span::styled("YEAR", Style::default().fg(p.text_dim)),
    ]));
    f.render_widget(footer, chunks[3]);
}

fn render_global_header(
    f: &mut Frame,
    p: &Palette,
    store: &HabitStore,
    today: NaiveDate,
    year: i32,
    area: Rect,
) {
    let year_start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap_or(today);
    let year_end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap_or(today);
    let range_end = if year_end > today { today } else { year_end };
    let mut total_completions: u64 = 0;
    let mut active_days = std::collections::BTreeSet::<NaiveDate>::new();
    for h in &store.habits {
        if !matches!(h.kind, HabitKind::Build) {
            continue;
        }
        for d in h.completions.range(year_start..=range_end) {
            total_completions += 1;
            active_days.insert(*d);
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.primary));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let dim_sep = Span::styled("   ·   ", Style::default().fg(p.primary_dark));

    let line = Line::from(vec![
        Span::styled(
            " GLOBAL ",
            Style::default()
                .fg(Color::Black)
                .bg(p.primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            "ACTIVITY ACROSS ALL HABITS",
            Style::default().fg(p.primary).add_modifier(Modifier::BOLD),
        ),
        dim_sep.clone(),
        Span::styled(format!("year {}", year), Style::default().fg(p.text_dim)),
        dim_sep.clone(),
        Span::styled(
            format!("{} completions", total_completions),
            Style::default().fg(p.text).add_modifier(Modifier::BOLD),
        ),
        dim_sep,
        Span::styled(
            format!("{} active days", active_days.len()),
            Style::default().fg(p.text).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line), inner);
}

fn render_global_summary(f: &mut Frame, p: &Palette, store: &HabitStore, today: NaiveDate, area: Rect) {
    let build_count = store
        .habits
        .iter()
        .filter(|h| matches!(h.kind, HabitKind::Build))
        .count();
    let quit_count = store.habits.len() - build_count;

    let done_today = store
        .habits
        .iter()
        .filter(|h| matches!(h.kind, HabitKind::Build) && h.completions.contains(&today))
        .count();

    let dim_sep = Span::styled("   ·   ", Style::default().fg(p.primary_dark));

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(format!("{} build", build_count), Style::default().fg(p.primary)),
        dim_sep.clone(),
        Span::styled(format!("{} quit", quit_count), Style::default().fg(p.primary_dim)),
        dim_sep,
        Span::styled(
            format!("{} completed today", done_today),
            Style::default().fg(C_AMBER).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_global_grid(
    f: &mut Frame,
    p: &Palette,
    store: &HabitStore,
    today: NaiveDate,
    year: i32,
    area: Rect,
) {
    let weeks: i64 = 26;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.primary_dark))
        .title(Span::styled(
            format!(" {} · COMBINED ACTIVITY ", year),
            Style::default().fg(p.primary_dim).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let year_end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap_or(today);
    let anchor = if year == today.year() { today } else { year_end.min(today) };
    let dow = anchor.weekday().num_days_from_monday() as i64;
    let week_end = anchor + Duration::days(6 - dow);
    let total_days = weeks * 7;
    let week_start = week_end - Duration::days(total_days - 1);

    let mut counts: BTreeMap<NaiveDate, u32> = BTreeMap::new();
    for h in &store.habits {
        if !matches!(h.kind, HabitKind::Build) {
            continue;
        }
        for d in h.completions.range(week_start..=anchor) {
            *counts.entry(*d).or_insert(0) += 1;
        }
    }
    let max_count = counts.values().copied().max().unwrap_or(0);

    let labels = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
    let mut lines: Vec<Line> = Vec::new();

    for row in 0..7 {
        let mut spans: Vec<Span> = Vec::with_capacity(weeks as usize + 2);
        spans.push(Span::styled(
            format!(" {}  ", labels[row]),
            Style::default().fg(p.text_dim),
        ));
        for col in 0..weeks as usize {
            let date = week_start + Duration::days((col as i64) * 7 + row as i64);
            spans.push(global_cell(p, date, today, year, &counts, max_count));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
        if row < 6 {
            lines.push(Line::from(""));
        }
    }

    lines.push(Line::from(""));
    lines.push(global_legend(p));

    f.render_widget(Paragraph::new(lines), inner);
}

fn global_cell(
    p: &Palette,
    date: NaiveDate,
    today: NaiveDate,
    year: i32,
    counts: &BTreeMap<NaiveDate, u32>,
    max_count: u32,
) -> Span<'static> {
    let cell = "\u{2588}\u{2588}";
    if date > today {
        return Span::styled("  ", Style::default());
    }
    if date.year() != year {
        return Span::styled(cell, Style::default().fg(p.primary_faint));
    }
    let n = counts.get(&date).copied().unwrap_or(0);
    let level = if max_count == 0 || n == 0 {
        0
    } else {
        let ratio = (n as f32) / (max_count as f32);
        if ratio >= 0.75 { 4 }
        else if ratio >= 0.5 { 3 }
        else if ratio >= 0.25 { 2 }
        else { 1 }
    };
    Span::styled(cell, Style::default().fg(level_color(p, level)))
}

fn level_color(p: &Palette, level: u8) -> Color {
    match level {
        0 => p.primary_faint,
        1 => p.level_1,
        2 => p.level_2,
        3 => p.level_3,
        _ => p.level_4,
    }
}

fn global_legend(p: &Palette) -> Line<'static> {
    let cell = "\u{2588}\u{2588}";
    Line::from(vec![
        Span::raw("    "),
        Span::styled("Less ", Style::default().fg(p.text_dim)),
        Span::styled(cell, Style::default().fg(level_color(p, 0))),
        Span::raw(" "),
        Span::styled(cell, Style::default().fg(level_color(p, 1))),
        Span::raw(" "),
        Span::styled(cell, Style::default().fg(level_color(p, 2))),
        Span::raw(" "),
        Span::styled(cell, Style::default().fg(level_color(p, 3))),
        Span::raw(" "),
        Span::styled(cell, Style::default().fg(level_color(p, 4))),
        Span::styled(" More", Style::default().fg(p.text_dim)),
    ])
}
