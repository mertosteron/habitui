use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Sparkline, Table, TableState, Wrap,
};
use ratatui::Frame;

use crate::data::{Frequency, Habit, HabitKind, HabitStore};
use crate::tui::app::{
    AddForm, App, DetailState, EditForm, FormField, FrequencyChoice, KindChoice, Screen,
};

const CELL: &str = "\u{2588}\u{2588}"; // "██"
const CELL_HALF: &str = "\u{2592}\u{2592}"; // shaded block

// Phosphor green terminal palette.
const C_GREEN: Color = Color::Rgb(80, 240, 130);
const C_GREEN_DIM: Color = Color::Rgb(50, 160, 90);
const C_GREEN_DARK: Color = Color::Rgb(30, 90, 55);
const C_GREEN_FAINT: Color = Color::Rgb(20, 55, 35);
const C_GREEN_BG: Color = Color::Rgb(12, 38, 24);
const C_TEXT: Color = Color::Rgb(150, 230, 170);
const C_TEXT_DIM: Color = Color::Rgb(80, 130, 95);
const C_RED: Color = Color::Rgb(230, 95, 95);
const C_AMBER: Color = Color::Rgb(240, 190, 90);

const PROGRESS_DAYS: i64 = 30;

pub fn render(f: &mut Frame, app: &mut App) {
    match &app.screen {
        Screen::List => render_list(f, app),
        Screen::AddHabit(_) => {
            render_list(f, app);
            if let Screen::AddHabit(form) = &app.screen {
                render_add_form(f, form);
            }
        }
        Screen::EditHabit(_) => {
            render_list(f, app);
            if let Screen::EditHabit(form) = &app.screen {
                render_edit_form(f, form);
            }
        }
        Screen::Detail(state) => {
            render_detail(f, app, state);
        }
        Screen::GlobalHeatmap => render_global_heatmap(f, app),
        Screen::ConfirmDelete { habit_id } => {
            let id = *habit_id;
            render_list(f, app);
            render_confirm_delete(f, app, id);
        }
    }
}

pub fn render_resize_notice(f: &mut Frame, area: Rect) {
    let msg = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "Please resize to at least 80x24",
            Style::default().fg(C_AMBER).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "(Press Ctrl-C to quit.)",
            Style::default().fg(C_TEXT_DIM),
        )),
    ])
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true });
    f.render_widget(msg, area);
}

// ---------- List screen ----------

fn render_list(f: &mut Frame, app: &mut App) {
    let outer = f.area();

    // Side gutters give the screen breathing room, like the reference.
    let h_pad = (outer.width / 24).max(2);
    let inner_w = outer.width.saturating_sub(h_pad * 2);
    let inner = Rect::new(outer.x + h_pad, outer.y + 1, inner_w, outer.height.saturating_sub(2));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // HABITUI title
            Constraint::Length(1), // date subtitle
            Constraint::Length(2), // spacer
            Constraint::Length(1), // column headers
            Constraint::Length(1), // spacer
            Constraint::Min(3),    // habit table
            Constraint::Length(1), // status (transient)
            Constraint::Length(1), // footer status bar
        ])
        .split(inner);

    // Title
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "HABITUI",
            Style::default()
                .fg(C_GREEN)
                .add_modifier(Modifier::BOLD),
        ))),
        chunks[0],
    );

    // Date subtitle
    let date_line = format_long_date(app.today).to_uppercase();
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            date_line,
            Style::default().fg(C_GREEN_DIM),
        ))),
        chunks[1],
    );

    // Habit table
    render_habit_table(f, app, chunks[3], chunks[5]);

    // Transient status
    let status_line: Line = match &app.status {
        Some(msg) => Line::from(Span::styled(
            msg.clone(),
            Style::default().fg(C_AMBER),
        )),
        None => Line::from(""),
    };
    f.render_widget(Paragraph::new(status_line), chunks[6]);

    // Footer status bar
    f.render_widget(render_status_bar(app), chunks[7]);
}

fn render_habit_table(f: &mut Frame, app: &mut App, header_area: Rect, body_area: Rect) {
    let name_w: u16 = 22;
    let progress_w: u16 = (PROGRESS_DAYS as u16) + 2;
    let streak_w: u16 = 10;

    // Column headers (rendered above the table for full control over spacing).
    // Gutter width matches the table's highlight_symbol width below ("▶  " = 3).
    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(3), // selection arrow gutter
            Constraint::Length(name_w),
            Constraint::Length(progress_w),
            Constraint::Min(streak_w),
        ])
        .split(header_area);

    let header_style = Style::default().fg(C_TEXT_DIM).add_modifier(Modifier::BOLD);
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
            Line::from(Span::styled("STREAK", header_style))
                .alignment(Alignment::Right),
        ),
        header_layout[3],
    );

    // Body rows
    if app.store.habits.is_empty() {
        let empty = Paragraph::new(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                "(no habits yet — press [a] to add one)",
                Style::default().fg(C_TEXT_DIM),
            ),
        ]));
        f.render_widget(empty, body_area);
        return;
    }

    let rows: Vec<Row> = app
        .store
        .habits
        .iter()
        .map(|h| habit_row(h, app.today, name_w as usize, PROGRESS_DAYS as usize))
        .collect();

    let widths = [
        Constraint::Length(name_w),
        Constraint::Length(progress_w),
        Constraint::Min(streak_w),
    ];
    let table = Table::new(rows, widths)
        .column_spacing(0)
        .highlight_style(
            Style::default()
                .bg(C_GREEN_BG)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(Span::styled(
            "▶  ",
            Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD),
        ));

    let mut state = TableState::default();
    state.select(Some(app.selected));
    f.render_stateful_widget(table, body_area, &mut state);
}

fn habit_row(h: &Habit, today: NaiveDate, name_w: usize, days: usize) -> Row<'static> {
    let is_quit = matches!(h.kind, HabitKind::Quit { .. });

    // Status dot/diamond before the name.
    let (dot, dot_color) = if is_quit {
        ("◆", C_GREEN_DIM)
    } else {
        ("●", C_GREEN)
    };

    // Name truncated/padded.
    let name = truncate(&h.name, name_w.saturating_sub(3));

    let name_line = Line::from(vec![
        Span::styled(dot, Style::default().fg(dot_color)),
        Span::raw(" "),
        Span::styled(
            name,
            Style::default().fg(C_TEXT).add_modifier(Modifier::BOLD),
        ),
    ]);

    // 30-day progress bar.
    let progress_line = Line::from(progress_bar_spans(h, today, days));

    // Streak / days-free.
    let streak_line = streak_line(h, today, is_quit);

    Row::new(vec![
        Cell::from(name_line),
        Cell::from(progress_line),
        Cell::from(streak_line.alignment(Alignment::Right)),
    ])
    .height(1)
}

fn progress_bar_spans(h: &Habit, today: NaiveDate, days: usize) -> Vec<Span<'static>> {
    let is_quit = matches!(h.kind, HabitKind::Quit { .. });
    let start = today - Duration::days(days as i64 - 1);
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(days);
    let mut d = start;
    for _ in 0..days {
        let span = if d < h.created_at {
            Span::styled("░", Style::default().fg(C_GREEN_FAINT))
        } else {
            let good = match &h.kind {
                HabitKind::Build => h.completions.contains(&d),
                HabitKind::Quit { failures } => !failures.contains(&d),
            };
            if good {
                Span::styled("█", Style::default().fg(C_GREEN))
            } else if is_quit {
                Span::styled("█", Style::default().fg(C_RED))
            } else {
                Span::styled("▒", Style::default().fg(C_GREEN_DARK))
            }
        };
        spans.push(span);
        d = d + Duration::days(1);
    }
    spans
}

fn streak_line(h: &Habit, today: NaiveDate, is_quit: bool) -> Line<'static> {
    let streak = h.current_streak(today);
    if is_quit {
        Line::from(vec![Span::styled(
            format!("{}d free", streak),
            Style::default().fg(C_GREEN_DIM),
        )])
    } else if streak == 0 {
        Line::from(vec![Span::styled(
            "—",
            Style::default().fg(C_TEXT_DIM),
        )])
    } else {
        Line::from(vec![
            Span::styled(
                format!("{}", streak),
                Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD),
            ),
            Span::raw("🔥"),
        ])
    }
}

fn render_status_bar<'a>(app: &App) -> Paragraph<'a> {
    // Total Build habits scheduled today: anything that's due *or* already
    // completed today. `is_due` returns false once a habit has been completed,
    // so we OR the two to get the true scheduled count.
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
        .filter(|h| {
            matches!(h.kind, HabitKind::Build) && h.completions.contains(&app.today)
        })
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
        Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(sep, Style::default().fg(C_GREEN_DARK)));
    spans.push(Span::styled(
        format!("{}", app.today),
        Style::default().fg(C_GREEN_DIM),
    ));
    spans.push(Span::styled(sep, Style::default().fg(C_GREEN_DARK)));
    spans.push(Span::styled(
        format!("{}/{} today", done_today, scheduled_today),
        Style::default().fg(C_GREEN_DIM),
    ));
    spans.push(Span::styled(sep, Style::default().fg(C_GREEN_DARK)));
    extend_spans(&mut spans, key_hint("↑↓/jk", "NAV"));
    spans.push(Span::raw(" "));
    if current_is_quit {
        extend_spans(&mut spans, key_hint("F", "FAIL"));
    } else {
        extend_spans(&mut spans, key_hint("SPACE", "CHECK"));
    }
    spans.push(Span::raw(" "));
    extend_spans(&mut spans, key_hint("ENTER", "GRAPH"));
    spans.push(Span::raw(" "));
    extend_spans(&mut spans, key_hint("A", "ADD"));
    spans.push(Span::raw(" "));
    extend_spans(&mut spans, key_hint("E", "EDIT"));
    spans.push(Span::raw(" "));
    extend_spans(&mut spans, key_hint("D", "DEL"));
    spans.push(Span::raw(" "));
    extend_spans(&mut spans, key_hint("G", "GLOBAL"));

    Paragraph::new(Line::from(spans))
}

fn key_hint<'a>(key: &str, label: &str) -> Vec<Span<'a>> {
    vec![
        Span::styled(
            key.to_string(),
            Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            label.to_string(),
            Style::default().fg(C_TEXT_DIM),
        ),
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
    // e.g. "Friday, May 1, 2026"
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

fn render_add_form(f: &mut Frame, form: &AddForm) {
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
        .border_style(Style::default().fg(C_GREEN))
        .title(Span::styled(
            " NEW HABIT ",
            Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD),
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

        f.render_widget(field_label("Name", form.field == FormField::Name), layout[0]);
        f.render_widget(name_paragraph(&form.name, form.field == FormField::Name), layout[1]);
        f.render_widget(field_label("Type", form.field == FormField::Kind), layout[3]);
        f.render_widget(kind_picker(form.kind_choice), layout[4]);
        f.render_widget(quit_implicit_daily_note(), layout[6]);
        f.render_widget(form_help(form.error.as_deref(), "Tab to switch · Enter to save · Esc to cancel"), layout[8]);
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

    f.render_widget(field_label("Name", form.field == FormField::Name), layout[0]);
    f.render_widget(name_paragraph(&form.name, form.field == FormField::Name), layout[1]);

    f.render_widget(field_label("Type", form.field == FormField::Kind), layout[3]);
    f.render_widget(kind_picker(form.kind_choice), layout[4]);

    f.render_widget(field_label("Frequency", form.field == FormField::Frequency), layout[6]);
    f.render_widget(freq_picker(form.freq_choice), layout[7]);

    if form.freq_choice.needs_numeric() {
        f.render_widget(
            numeric_field(form.freq_choice, &form.numeric_buf, form.field == FormField::NumericValue),
            layout[8],
        );
    }

    f.render_widget(form_help(form.error.as_deref(), "Tab to switch · Enter to save · Esc to cancel"), layout[10]);
}

fn quit_implicit_daily_note<'a>() -> Paragraph<'a> {
    Paragraph::new(Line::from(vec![
        Span::styled("  Frequency: ", Style::default().fg(C_TEXT_DIM)),
        Span::styled(
            "Daily (implicit for Quit habits)",
            Style::default()
                .fg(C_TEXT)
                .add_modifier(Modifier::ITALIC),
        ),
    ]))
}

// ---------- Edit form ----------

fn render_edit_form(f: &mut Frame, form: &EditForm) {
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

        f.render_widget(field_label("Name", form.field == FormField::Name), layout[0]);
        f.render_widget(name_paragraph(&form.name, form.field == FormField::Name), layout[1]);
        f.render_widget(field_label("Type (read-only)", false), layout[3]);
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    form.kind_label,
                    Style::default().fg(C_TEXT).add_modifier(Modifier::ITALIC),
                ),
            ])),
            layout[4],
        );
        f.render_widget(quit_implicit_daily_note(), layout[6]);
        f.render_widget(form_help(form.error.as_deref(), "Enter to save · Esc to cancel"), layout[8]);
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

    f.render_widget(field_label("Name", form.field == FormField::Name), layout[0]);
    f.render_widget(name_paragraph(&form.name, form.field == FormField::Name), layout[1]);

    f.render_widget(field_label("Type (read-only)", false), layout[3]);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                form.kind_label,
                Style::default().fg(C_TEXT).add_modifier(Modifier::ITALIC),
            ),
        ])),
        layout[4],
    );

    f.render_widget(field_label("Frequency", form.field == FormField::Frequency), layout[6]);
    f.render_widget(freq_picker(form.freq_choice), layout[7]);

    if form.freq_choice.needs_numeric() {
        f.render_widget(
            numeric_field(form.freq_choice, &form.numeric_buf, form.field == FormField::NumericValue),
            layout[8],
        );
    }

    f.render_widget(form_help(form.error.as_deref(), "Tab to switch · Enter to save · Esc to cancel"), layout[10]);
}

fn field_label<'a>(text: &'a str, focused: bool) -> Paragraph<'a> {
    let style = if focused {
        Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(C_TEXT_DIM)
    };
    Paragraph::new(Span::styled(text, style))
}

fn name_paragraph<'a>(name: &'a str, focused: bool) -> Paragraph<'a> {
    let display = if name.is_empty() {
        Span::styled(" <type a name>", Style::default().fg(C_TEXT_DIM))
    } else {
        Span::styled(format!(" {}", name), Style::default().fg(C_TEXT))
    };
    let style = if focused {
        Style::default().bg(C_GREEN_BG)
    } else {
        Style::default()
    };
    Paragraph::new(Line::from(vec![display])).style(style)
}

fn kind_picker<'a>(choice: KindChoice) -> Paragraph<'a> {
    let mut spans: Vec<Span> = Vec::new();
    for (c, label) in [
        (KindChoice::Build, "[Build]"),
        (KindChoice::Quit, "[Quit]"),
    ] {
        let selected = c == choice;
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(C_GREEN)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_TEXT_DIM)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    Paragraph::new(Line::from(spans))
}

fn freq_picker<'a>(choice: FrequencyChoice) -> Paragraph<'a> {
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
                .bg(C_GREEN)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_TEXT_DIM)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    Paragraph::new(Line::from(spans))
}

fn numeric_field<'a>(choice: FrequencyChoice, buf: &str, focused: bool) -> Paragraph<'a> {
    let label = match choice {
        FrequencyChoice::EveryNDays => "Days (N)",
        FrequencyChoice::NTimesPerWeek => "Times per week (N)",
        _ => "N",
    };
    let label_style = if focused {
        Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(C_TEXT_DIM)
    };
    let value_style = if focused {
        Style::default()
            .bg(C_GREEN_BG)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(C_TEXT)
    };
    let value_display = if buf.is_empty() {
        Span::styled(" _ ", Style::default().fg(C_TEXT_DIM))
    } else {
        Span::styled(format!(" {} ", buf), value_style)
    };
    Paragraph::new(Line::from(vec![
        Span::styled(format!("{}: ", label), label_style),
        value_display,
        Span::styled(
            "  digits to type · backspace to clear",
            Style::default().fg(C_TEXT_DIM),
        ),
    ]))
}

fn form_help<'a>(err: Option<&'a str>, fallback: &'a str) -> Paragraph<'a> {
    let line = match err {
        Some(e) => Line::from(Span::styled(
            e.to_string(),
            Style::default().fg(C_RED).add_modifier(Modifier::BOLD),
        )),
        None => Line::from(Span::styled(
            fallback.to_string(),
            Style::default().fg(C_TEXT_DIM),
        )),
    };
    Paragraph::new(line)
}

// ---------- Confirm-delete ----------

fn render_confirm_delete(f: &mut Frame, app: &App, habit_id: u64) {
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
            Span::styled("Delete \"", Style::default().fg(C_TEXT)),
            Span::styled(name, Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("\" ?", Style::default().fg(C_TEXT)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Y] ", Style::default().fg(C_RED).add_modifier(Modifier::BOLD)),
            Span::styled("yes  ", Style::default().fg(C_TEXT)),
            Span::styled("[N] ", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("no", Style::default().fg(C_TEXT)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
}

// ---------- Detail / binary chart ----------

fn render_detail(f: &mut Frame, app: &App, state: &DetailState) {
    let area = f.area();
    let habit = match app.store.habit(state.habit_id) {
        Some(h) => h,
        None => {
            f.render_widget(
                Paragraph::new(Span::styled(
                    "Habit not found. Press q to return.",
                    Style::default().fg(C_TEXT),
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

    render_detail_header(f, habit, app.today, chunks[0]);
    render_motivation_line(f, habit, app.today, chunks[1]);
    render_recent_strip(f, habit, app.today, app.year, chunks[2]);
    render_binary_calendar(f, habit, app.today, app.year, state, chunks[3]);

    let status_line = match &app.status {
        Some(msg) => Line::from(Span::styled(
            format!("  {}", msg),
            Style::default().fg(C_AMBER),
        )),
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
            Span::styled("←↑↓→ ", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("MOVE", Style::default().fg(C_TEXT_DIM)),
            Span::styled("    ", Style::default()),
            Span::styled("SPACE ", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("TOGGLE", Style::default().fg(C_TEXT_DIM)),
            Span::styled("    ", Style::default()),
            Span::styled("E/ESC ", Style::default().fg(C_RED).add_modifier(Modifier::BOLD)),
            Span::styled("EXIT EDIT", Style::default().fg(C_TEXT_DIM)),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled("  ESC/Q/ENTER ", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("BACK", Style::default().fg(C_TEXT_DIM)),
            Span::styled("    ", Style::default()),
            Span::styled("E ", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("EDIT PAST DAYS", Style::default().fg(C_TEXT_DIM)),
            Span::styled("    ", Style::default()),
            Span::styled("[ / ] ", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("YEAR", Style::default().fg(C_TEXT_DIM)),
        ]))
    };
    f.render_widget(footer, chunks[5]);
}

fn render_recent_strip(f: &mut Frame, habit: &Habit, today: NaiveDate, year: i32, area: Rect) {
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });
    let title = if is_quit {
        format!(" {} · FAILURES BY DAY ", year)
    } else {
        format!(" {} · ACTIVITY BY DAY ", year)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(C_GREEN_DARK))
        .title(Span::styled(
            title,
            Style::default().fg(C_GREEN_DIM).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let year_start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap_or(today);
    let year_end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap_or(today);
    let end = if year_end > today { today } else { year_end };
    let mut data: Vec<u64> = Vec::new();
    let mut d = year_start;
    while d <= end {
        if d < habit.created_at {
            data.push(0);
        } else {
            let hit = match &habit.kind {
                HabitKind::Build => habit.completions.contains(&d),
                HabitKind::Quit { failures } => failures.contains(&d),
            };
            data.push(if hit { 1 } else { 0 });
        }
        d = d + Duration::days(1);
    }

    let bar_color = if is_quit { C_RED } else { C_GREEN };

    let sparkline = Sparkline::default()
        .data(&data)
        .max(1)
        .bar_set(symbols::bar::NINE_LEVELS)
        .style(Style::default().fg(bar_color));
    f.render_widget(sparkline, inner);
}

fn render_binary_calendar(
    f: &mut Frame,
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
    let border_color = if state.edit_mode { C_AMBER } else { C_GREEN_DARK };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default().fg(C_GREEN_DIM).add_modifier(Modifier::BOLD),
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
            Style::default().fg(C_TEXT_DIM),
        ));
        for col in 0..weeks as usize {
            let date = week_start + Duration::days((col as i64) * 7 + row as i64);
            spans.push(binary_cell(habit, date, today, year, state));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
        if row < 6 {
            lines.push(Line::from(""));
        }
    }

    lines.push(Line::from(""));
    lines.push(binary_legend(is_quit));

    f.render_widget(Paragraph::new(lines), inner);
}

fn binary_cell(
    habit: &Habit,
    date: NaiveDate,
    today: NaiveDate,
    year: i32,
    state: &DetailState,
) -> Span<'static> {
    let is_cursor = state.edit_mode && date == state.cursor;
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });

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
        let s = Style::default().fg(C_GREEN_FAINT);
        let s = if is_cursor { s.add_modifier(Modifier::REVERSED) } else { s };
        return Span::styled(CELL, s);
    }
    if date < habit.created_at {
        let s = Style::default().fg(C_GREEN_FAINT);
        let s = if is_cursor { s.add_modifier(Modifier::REVERSED) } else { s };
        return Span::styled(CELL, s);
    }

    let hit = match &habit.kind {
        HabitKind::Build => habit.completions.contains(&date),
        HabitKind::Quit { failures } => failures.contains(&date),
    };

    let (glyph, color) = if is_quit {
        if hit {
            (CELL_HALF, C_RED)
        } else {
            (CELL, C_GREEN_DIM)
        }
    } else if hit {
        (CELL, C_GREEN)
    } else {
        (CELL, C_GREEN_DARK)
    };

    let mut style = Style::default().fg(color);
    if is_cursor {
        style = style
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::REVERSED);
    }
    Span::styled(glyph, style)
}

fn binary_legend(is_quit: bool) -> Line<'static> {
    if is_quit {
        Line::from(vec![
            Span::raw("    "),
            Span::styled("Legend: ", Style::default().fg(C_TEXT_DIM)),
            Span::styled(CELL, Style::default().fg(C_GREEN_DIM)),
            Span::styled(" clean   ", Style::default().fg(C_TEXT_DIM)),
            Span::styled(CELL_HALF, Style::default().fg(C_RED)),
            Span::styled(" failure   ", Style::default().fg(C_TEXT_DIM)),
            Span::styled(CELL, Style::default().fg(C_GREEN_FAINT)),
            Span::styled(" before created", Style::default().fg(C_TEXT_DIM)),
        ])
    } else {
        Line::from(vec![
            Span::raw("    "),
            Span::styled("Legend: ", Style::default().fg(C_TEXT_DIM)),
            Span::styled(CELL, Style::default().fg(C_GREEN)),
            Span::styled(" done   ", Style::default().fg(C_TEXT_DIM)),
            Span::styled(CELL, Style::default().fg(C_GREEN_DARK)),
            Span::styled(" missed   ", Style::default().fg(C_TEXT_DIM)),
            Span::styled(CELL, Style::default().fg(C_GREEN_FAINT)),
            Span::styled(" before created", Style::default().fg(C_TEXT_DIM)),
        ])
    }
}

fn render_detail_header(f: &mut Frame, habit: &Habit, today: NaiveDate, area: Rect) {
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
        .border_style(Style::default().fg(C_GREEN_DARK));
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
                .bg(C_GREEN_DIM)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " BUILD ",
            Style::default()
                .fg(Color::Black)
                .bg(C_GREEN)
                .add_modifier(Modifier::BOLD),
        )
    };

    let streak_label = if is_quit {
        format!("{} days clean", current)
    } else {
        format!("{} day streak", current)
    };

    let dim_sep = Span::styled("   ·   ", Style::default().fg(C_GREEN_DARK));

    let line = Line::from(vec![
        kind_tag,
        Span::raw("  "),
        Span::styled(
            habit.name.clone(),
            Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(format_frequency(habit.frequency), Style::default().fg(C_TEXT_DIM)),
        dim_sep.clone(),
        Span::styled(
            streak_label,
            Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD),
        ),
        dim_sep.clone(),
        Span::styled("longest ", Style::default().fg(C_TEXT_DIM)),
        Span::styled(
            format!("{}", longest),
            Style::default().fg(C_TEXT).add_modifier(Modifier::BOLD),
        ),
        dim_sep,
        Span::styled(
            if is_quit { "failures " } else { "total " },
            Style::default().fg(C_TEXT_DIM),
        ),
        Span::styled(
            format!("{}", total),
            Style::default().fg(C_TEXT).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line), layout[0]);
}

fn render_motivation_line(f: &mut Frame, habit: &Habit, today: NaiveDate, area: Rect) {
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });
    let streak = habit.current_streak(today);
    let msg = affirmation(streak, is_quit);
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            msg,
            Style::default().fg(C_GREEN_DIM).add_modifier(Modifier::ITALIC),
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

fn render_global_heatmap(f: &mut Frame, app: &App) {
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

    render_global_header(f, &app.store, app.today, app.year, chunks[0]);
    render_global_summary(f, &app.store, app.today, chunks[1]);
    render_global_grid(f, &app.store, app.today, app.year, chunks[2]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("  ESC/Q/G/ENTER ", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)),
        Span::styled("BACK TO LIST", Style::default().fg(C_TEXT_DIM)),
        Span::styled("    ", Style::default()),
        Span::styled("[ / ] ", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD)),
        Span::styled("YEAR", Style::default().fg(C_TEXT_DIM)),
    ]));
    f.render_widget(footer, chunks[3]);
}

fn render_global_header(
    f: &mut Frame,
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
        .border_style(Style::default().fg(C_GREEN));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let dim_sep = Span::styled("   ·   ", Style::default().fg(C_GREEN_DARK));

    let line = Line::from(vec![
        Span::styled(
            " GLOBAL ",
            Style::default()
                .fg(Color::Black)
                .bg(C_GREEN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            "ACTIVITY ACROSS ALL HABITS",
            Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD),
        ),
        dim_sep.clone(),
        Span::styled(format!("year {}", year), Style::default().fg(C_TEXT_DIM)),
        dim_sep.clone(),
        Span::styled(
            format!("{} completions", total_completions),
            Style::default().fg(C_TEXT).add_modifier(Modifier::BOLD),
        ),
        dim_sep,
        Span::styled(
            format!("{} active days", active_days.len()),
            Style::default().fg(C_TEXT).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line), inner);
}

fn render_global_summary(f: &mut Frame, store: &HabitStore, today: NaiveDate, area: Rect) {
    let build_count = store
        .habits
        .iter()
        .filter(|h| matches!(h.kind, HabitKind::Build))
        .count();
    let quit_count = store.habits.len() - build_count;

    let done_today = store
        .habits
        .iter()
        .filter(|h| {
            matches!(h.kind, HabitKind::Build) && h.completions.contains(&today)
        })
        .count();

    let dim_sep = Span::styled("   ·   ", Style::default().fg(C_GREEN_DARK));

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(format!("{} build", build_count), Style::default().fg(C_GREEN)),
        dim_sep.clone(),
        Span::styled(format!("{} quit", quit_count), Style::default().fg(C_GREEN_DIM)),
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
    store: &HabitStore,
    today: NaiveDate,
    year: i32,
    area: Rect,
) {
    let weeks: i64 = 26;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(C_GREEN_DARK))
        .title(Span::styled(
            format!(" {} · COMBINED ACTIVITY ", year),
            Style::default().fg(C_GREEN_DIM).add_modifier(Modifier::BOLD),
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
            Style::default().fg(C_TEXT_DIM),
        ));
        for col in 0..weeks as usize {
            let date = week_start + Duration::days((col as i64) * 7 + row as i64);
            spans.push(global_cell(date, today, year, &counts, max_count));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
        if row < 6 {
            lines.push(Line::from(""));
        }
    }

    lines.push(Line::from(""));
    lines.push(global_legend());

    f.render_widget(Paragraph::new(lines), inner);
}

fn global_cell(
    date: NaiveDate,
    today: NaiveDate,
    year: i32,
    counts: &BTreeMap<NaiveDate, u32>,
    max_count: u32,
) -> Span<'static> {
    if date > today {
        return Span::styled("  ", Style::default());
    }
    if date.year() != year {
        return Span::styled(CELL, Style::default().fg(C_GREEN_FAINT));
    }
    let n = counts.get(&date).copied().unwrap_or(0);
    let level = if max_count == 0 || n == 0 {
        0
    } else {
        let ratio = (n as f32) / (max_count as f32);
        if ratio >= 0.75 {
            4
        } else if ratio >= 0.5 {
            3
        } else if ratio >= 0.25 {
            2
        } else {
            1
        }
    };
    Span::styled(CELL, Style::default().fg(global_palette(level)))
}

fn global_palette(level: u8) -> Color {
    match level {
        0 => C_GREEN_FAINT,
        1 => Color::Rgb(40, 110, 70),
        2 => Color::Rgb(60, 170, 100),
        3 => Color::Rgb(90, 220, 130),
        _ => Color::Rgb(140, 255, 170),
    }
}

fn global_legend() -> Line<'static> {
    Line::from(vec![
        Span::raw("    "),
        Span::styled("Less ", Style::default().fg(C_TEXT_DIM)),
        Span::styled(CELL, Style::default().fg(global_palette(0))),
        Span::raw(" "),
        Span::styled(CELL, Style::default().fg(global_palette(1))),
        Span::raw(" "),
        Span::styled(CELL, Style::default().fg(global_palette(2))),
        Span::raw(" "),
        Span::styled(CELL, Style::default().fg(global_palette(3))),
        Span::raw(" "),
        Span::styled(CELL, Style::default().fg(global_palette(4))),
        Span::styled(" More", Style::default().fg(C_TEXT_DIM)),
    ])
}
