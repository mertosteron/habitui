use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDate};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Sparkline, Wrap,
};
use ratatui::Frame;

use crate::data::{Frequency, Habit, HabitKind, HabitStore};
use crate::tui::app::{
    AddForm, App, DetailState, EditForm, FormField, FrequencyChoice, KindChoice, Screen,
};

const CELL: &str = "\u{2588}\u{2588}"; // "██"
const CELL_HALF: &str = "\u{2592}\u{2592}"; // shaded block, used for "logged failure"

// Color category palette (action grouping for footer + motivation).
const COL_NAV: Color = Color::Rgb(120, 170, 220);     // soft blue
const COL_MUT: Color = Color::Rgb(120, 200, 140);     // muted green
const COL_DANGER: Color = Color::Rgb(220, 110, 110);  // soft red
const COL_QUIT_BG: Color = Color::Rgb(180, 180, 180); // gray
const COL_DIM: Color = Color::Rgb(110, 110, 130);     // dimmer separators
const COL_ACCENT: Color = Color::Rgb(245, 200, 90);   // honeyed amber
const COL_HEADER: Color = Color::Rgb(180, 200, 240);

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
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("(Press Ctrl-C to quit.)"),
    ])
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true });
    f.render_widget(msg, area);
}

fn render_list(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Min(3),    // list
            Constraint::Length(1), // status (transient)
            Constraint::Length(1), // keybindings
        ])
        .split(f.area());

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "✦ ",
            Style::default().fg(COL_ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "habitui",
            Style::default()
                .fg(COL_ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
        Span::styled(
            "   ",
            Style::default(),
        ),
        Span::styled(
            format!("{}", app.today.format("%a %Y-%m-%d")),
            Style::default().fg(COL_HEADER),
        ),
        Span::styled(
            format!("   ·   {} habit{}", app.store.habits.len(),
                if app.store.habits.len() == 1 { "" } else { "s" }),
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    f.render_widget(title, chunks[0]);

    let header = "  NAME                          FREQUENCY        STREAK    TODAY";
    let mut items: Vec<ListItem> = Vec::with_capacity(app.store.habits.len() + 1);
    items.push(ListItem::new(Line::from(Span::styled(
        header,
        Style::default().fg(Color::DarkGray),
    ))));

    if app.store.habits.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  (no habits yet — press [a] to add one)",
            Style::default().fg(Color::DarkGray),
        ))));
    } else {
        for h in &app.store.habits {
            items.push(ListItem::new(habit_row(h, app.today)));
        }
    }

    let mut list_state = ListState::default();
    if !app.store.habits.is_empty() {
        list_state.select(Some(app.selected + 1)); // +1 for header row
    }

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COL_DIM))
                .title(Span::styled(
                    " Habits ",
                    Style::default().fg(COL_HEADER).add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 45, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("› ");
    f.render_stateful_widget(list, chunks[1], &mut list_state);

    // Status: transient one-keypress-lifetime feedback.
    let status_line: Line = match &app.status {
        Some(msg) => Line::from(Span::styled(
            msg.clone(),
            Style::default().fg(COL_ACCENT),
        )),
        None => Line::from(""),
    };
    f.render_widget(Paragraph::new(status_line), chunks[2]);

    // Keybindings bar — context-aware: show [f] when current habit is Quit.
    let current_is_quit = app
        .store
        .habits
        .get(app.selected)
        .map(|h| matches!(h.kind, HabitKind::Quit { .. }))
        .unwrap_or(false);
    let footer = render_footer(current_is_quit);
    f.render_widget(footer, chunks[3]);
}

fn habit_row(h: &Habit, today: NaiveDate) -> Line<'static> {
    let name = pad(&h.name, 30);
    let freq = pad(&format_frequency(h.frequency), 16);
    let streak = h.current_streak(today);
    let is_quit = matches!(h.kind, HabitKind::Quit { .. });

    // Streak with motivation glyph that intensifies with thresholds.
    let (glyph, glyph_color) = streak_glyph(streak, is_quit);
    let streak_str = format!("{}{:<5}", glyph, streak);

    // TODAY column varies by kind/frequency.
    let (today_glyph, today_color) = today_indicator(h, today);

    Line::from(vec![
        Span::raw("  "),
        Span::raw(name),
        Span::raw(freq),
        Span::styled(streak_str, Style::default().fg(glyph_color).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled(today_glyph, Style::default().fg(today_color)),
    ])
}

/// Returns (glyph, color) for streak motivation tier.
/// Tiers: 0 (gray dot), 1-2 (dim), 3+ (spark), 7+ (flame), 14+ (bright flame),
/// 30+ (amber), 100+ (gold).
fn streak_glyph(streak: u32, is_quit: bool) -> (&'static str, Color) {
    // Quit habits use a different glyph (◆ / shield) so the visual category reads differently.
    let g = if is_quit {
        match streak {
            0 => "·",
            1..=2 => "◇",
            3..=6 => "◆",
            7..=13 => "◆",
            14..=29 => "◆",
            30..=99 => "◆",
            _ => "◆",
        }
    } else {
        match streak {
            0 => "·",
            1..=2 => "•",
            3..=6 => "✦",
            7..=13 => "✦",
            14..=29 => "✦",
            30..=99 => "✦",
            _ => "✦",
        }
    };
    let color = match streak {
        0 => Color::DarkGray,
        1..=2 => Color::Rgb(140, 140, 160),
        3..=6 => Color::Rgb(180, 200, 140),       // soft green
        7..=13 => Color::Rgb(220, 200, 100),      // mellow yellow
        14..=29 => Color::Rgb(240, 170, 90),      // amber
        30..=99 => Color::Rgb(245, 130, 90),      // orange-red
        _ => Color::Rgb(245, 90, 130),            // hot pink (legendary)
    };
    (g, color)
}

fn today_indicator(h: &Habit, today: NaiveDate) -> (String, Color) {
    match (&h.kind, h.frequency) {
        (HabitKind::Quit { failures }, _) => {
            if failures.contains(&today) {
                ("✗ failed".to_string(), COL_DANGER)
            } else {
                ("clean".to_string(), Color::Rgb(140, 200, 160))
            }
        }
        (HabitKind::Build, Frequency::NTimesPerWeek(n)) => {
            let monday = iso_week_monday(today);
            let sunday = monday + Duration::days(6);
            let count = h.completions.range(monday..=sunday).count() as u32;
            let color = if count >= n {
                Color::Rgb(140, 200, 160)
            } else {
                Color::Rgb(200, 180, 100)
            };
            (format!("{}/{} wk", count, n), color)
        }
        (HabitKind::Build, _) => {
            let done = h.completions.contains(&today);
            if done {
                ("✓".to_string(), Color::Rgb(140, 200, 160))
            } else if h.is_due(today) {
                ("·".to_string(), Color::DarkGray)
            } else {
                // satisfied via prior in-window completion
                ("✓".to_string(), Color::Rgb(120, 170, 200))
            }
        }
    }
}

fn iso_week_monday(date: NaiveDate) -> NaiveDate {
    let off = date.weekday().num_days_from_monday() as i64;
    date - Duration::days(off)
}

fn pad(s: &str, width: usize) -> String {
    let mut out = String::new();
    let mut len = 0;
    for c in s.chars() {
        if len >= width {
            break;
        }
        out.push(c);
        len += 1;
    }
    while len < width {
        out.push(' ');
        len += 1;
    }
    out
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

// ---------- Footer / keybindings bar ----------

fn render_footer<'a>(quit_habit_selected: bool) -> Paragraph<'a> {
    // Each binding rendered as `[K] label`, color-coded by category, joined
    // by a subdued middle-dot. Single line, professional.
    let sep = Span::styled(" · ", Style::default().fg(COL_DIM));

    let toggle = if quit_habit_selected {
        keybinding("f", "fail/clear", COL_DANGER)
    } else {
        keybinding("␣", "toggle", COL_MUT)
    };

    let mut spans: Vec<Span> = Vec::new();
    extend_spans(&mut spans, keybinding("j/k", "move", COL_NAV));
    spans.push(sep.clone());
    extend_spans(&mut spans, keybinding("⏎", "detail", COL_NAV));
    spans.push(sep.clone());
    extend_spans(&mut spans, keybinding("g", "global", COL_NAV));
    spans.push(sep.clone());
    extend_spans(&mut spans, toggle);
    spans.push(sep.clone());
    extend_spans(&mut spans, keybinding("a", "add", COL_MUT));
    spans.push(sep.clone());
    extend_spans(&mut spans, keybinding("e", "edit", COL_MUT));
    spans.push(sep.clone());
    extend_spans(&mut spans, keybinding("d", "delete", COL_DANGER));
    spans.push(sep);
    extend_spans(&mut spans, keybinding("q", "quit", COL_QUIT_BG));

    Paragraph::new(Line::from(spans))
}

fn extend_spans<'a>(out: &mut Vec<Span<'a>>, parts: Vec<Span<'a>>) {
    out.extend(parts);
}

fn keybinding<'a>(key: &str, label: &str, color: Color) -> Vec<Span<'a>> {
    vec![
        Span::styled("[", Style::default().fg(COL_DIM)),
        Span::styled(
            key.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("]", Style::default().fg(COL_DIM)),
        Span::raw(" "),
        Span::styled(label.to_string(), Style::default().fg(Color::Rgb(180, 180, 195))),
    ]
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
        .border_style(Style::default().fg(COL_HEADER))
        .title(Span::styled(
            " New habit ",
            Style::default().fg(COL_HEADER).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if is_quit {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // name label
                Constraint::Length(1), // name input
                Constraint::Length(1), // gap
                Constraint::Length(1), // kind label
                Constraint::Length(1), // kind picker
                Constraint::Length(1), // gap
                Constraint::Length(1), // implicit-daily note
                Constraint::Length(1), // gap
                Constraint::Min(1),    // help / error
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
            Constraint::Length(1), // name label
            Constraint::Length(1), // name input
            Constraint::Length(1), // gap
            Constraint::Length(1), // kind label
            Constraint::Length(1), // kind picker
            Constraint::Length(1), // gap
            Constraint::Length(1), // freq label
            Constraint::Length(1), // freq picker
            Constraint::Length(1), // numeric (when applicable)
            Constraint::Length(1), // gap
            Constraint::Min(1),    // help / error
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
        Span::styled("  Frequency: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "Daily (implicit for Quit habits)",
            Style::default()
                .fg(Color::Rgb(180, 180, 195))
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
        .border_style(Style::default().fg(COL_ACCENT))
        .title(Span::styled(
            " Edit habit ",
            Style::default().fg(COL_ACCENT).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if form.is_quit {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // name label
                Constraint::Length(1), // name input
                Constraint::Length(1), // gap
                Constraint::Length(1), // kind (read-only)
                Constraint::Length(1), // kind value
                Constraint::Length(1), // gap
                Constraint::Length(1), // implicit-daily note
                Constraint::Length(1), // gap
                Constraint::Min(1),    // help / error
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
                    Style::default().fg(Color::Rgb(180, 180, 195)).add_modifier(Modifier::ITALIC),
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
            Constraint::Length(1), // name label
            Constraint::Length(1), // name input
            Constraint::Length(1), // gap
            Constraint::Length(1), // kind (read-only)
            Constraint::Length(1), // kind value
            Constraint::Length(1), // gap
            Constraint::Length(1), // freq label
            Constraint::Length(1), // freq picker
            Constraint::Length(1), // numeric (when applicable)
            Constraint::Length(1), // gap
            Constraint::Min(1),    // help / error
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
                Style::default().fg(Color::Rgb(180, 180, 195)).add_modifier(Modifier::ITALIC),
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
        Style::default().fg(COL_HEADER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Paragraph::new(Span::styled(text, style))
}

fn name_paragraph<'a>(name: &'a str, focused: bool) -> Paragraph<'a> {
    let display = if name.is_empty() {
        Span::styled(" <type a name>", Style::default().fg(Color::DarkGray))
    } else {
        Span::raw(format!(" {}", name))
    };
    let style = if focused {
        Style::default().bg(Color::Rgb(40, 45, 60))
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
                .bg(COL_ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
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
                .bg(COL_HEADER)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
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
        Style::default().fg(COL_HEADER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let value_style = if focused {
        Style::default()
            .bg(Color::Rgb(40, 45, 60))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let value_display = if buf.is_empty() {
        Span::styled(" _ ", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(format!(" {} ", buf), value_style)
    };
    Paragraph::new(Line::from(vec![
        Span::styled(format!("{}: ", label), label_style),
        value_display,
        Span::styled(
            "  digits to type · backspace to clear",
            Style::default().fg(Color::DarkGray),
        ),
    ]))
}

fn form_help<'a>(err: Option<&'a str>, fallback: &'a str) -> Paragraph<'a> {
    let line = match err {
        Some(e) => Line::from(Span::styled(
            e.to_string(),
            Style::default().fg(COL_DANGER).add_modifier(Modifier::BOLD),
        )),
        None => Line::from(Span::styled(
            fallback.to_string(),
            Style::default().fg(Color::DarkGray),
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
        .border_style(Style::default().fg(COL_DANGER))
        .title(Span::styled(
            " Confirm delete ",
            Style::default().fg(COL_DANGER).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("Delete \""),
            Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("\" ?"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y] ", Style::default().fg(COL_DANGER).add_modifier(Modifier::BOLD)),
            Span::styled("yes  ", Style::default().fg(Color::Rgb(180, 180, 195))),
            Span::styled("[n] ", Style::default().fg(COL_NAV).add_modifier(Modifier::BOLD)),
            Span::styled("no", Style::default().fg(Color::Rgb(180, 180, 195))),
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
                Paragraph::new("Habit not found. Press q to return.").alignment(Alignment::Center),
                area,
            );
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header card
            Constraint::Length(2), // affirmation
            Constraint::Length(5), // sparkline strip (last 30 days)
            Constraint::Min(10),   // binary calendar
            Constraint::Length(1), // status
            Constraint::Length(1), // footer
        ])
        .split(area);

    render_detail_header(f, habit, app.today, chunks[0]);
    render_motivation_line(f, habit, app.today, chunks[1]);
    render_recent_strip(f, habit, app.today, chunks[2]);
    render_binary_calendar(f, habit, app.today, state, chunks[3]);

    let status_line = match &app.status {
        Some(msg) => Line::from(Span::styled(
            format!("  {}", msg),
            Style::default().fg(COL_ACCENT),
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
                    .bg(COL_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("[←↑↓→] ", Style::default().fg(COL_NAV).add_modifier(Modifier::BOLD)),
            Span::styled("move", Style::default().fg(Color::Rgb(180, 180, 195))),
            Span::styled("   ·   ", Style::default().fg(COL_DIM)),
            Span::styled("[space] ", Style::default().fg(COL_MUT).add_modifier(Modifier::BOLD)),
            Span::styled("toggle", Style::default().fg(Color::Rgb(180, 180, 195))),
            Span::styled("   ·   ", Style::default().fg(COL_DIM)),
            Span::styled("[e/esc] ", Style::default().fg(COL_DANGER).add_modifier(Modifier::BOLD)),
            Span::styled("exit edit", Style::default().fg(Color::Rgb(180, 180, 195))),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled("  [esc/q/⏎] ", Style::default().fg(COL_NAV).add_modifier(Modifier::BOLD)),
            Span::styled("back", Style::default().fg(Color::Rgb(180, 180, 195))),
            Span::styled("   ·   ", Style::default().fg(COL_DIM)),
            Span::styled("[e] ", Style::default().fg(COL_MUT).add_modifier(Modifier::BOLD)),
            Span::styled("edit past days", Style::default().fg(Color::Rgb(180, 180, 195))),
        ]))
    };
    f.render_widget(footer, chunks[5]);
}

fn render_recent_strip(f: &mut Frame, habit: &Habit, today: NaiveDate, area: Rect) {
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });
    let title = if is_quit {
        " Last 30 days · failures "
    } else {
        " Last 30 days · activity "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COL_DIM))
        .title(Span::styled(
            title,
            Style::default().fg(COL_HEADER).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Build 30 daily samples ending today: 0 or 1.
    let days = 30i64;
    let start = today - Duration::days(days - 1);
    let mut data: Vec<u64> = Vec::with_capacity(days as usize);
    for i in 0..days {
        let d = start + Duration::days(i);
        if d < habit.created_at {
            data.push(0);
            continue;
        }
        let hit = match &habit.kind {
            HabitKind::Build => habit.completions.contains(&d),
            HabitKind::Quit { failures } => failures.contains(&d),
        };
        data.push(if hit { 1 } else { 0 });
    }

    let bar_color = if is_quit {
        COL_DANGER
    } else {
        Color::Rgb(150, 220, 130)
    };

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
    state: &DetailState,
    area: Rect,
) {
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });
    let weeks: i64 = 18;

    let title = if state.edit_mode {
        format!(
            " {}-week binary view · editing {} ",
            weeks,
            state.cursor.format("%a %Y-%m-%d")
        )
    } else if is_quit {
        format!(" {}-week binary view · failures ", weeks)
    } else {
        format!(" {}-week binary view · done / missed ", weeks)
    };
    let border_color = if state.edit_mode { COL_ACCENT } else { COL_DIM };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default().fg(COL_HEADER).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Anchor: end of current ISO week (Sunday).
    let dow = today.weekday().num_days_from_monday() as i64;
    let week_end = today + Duration::days(6 - dow);
    let total_days = weeks * 7;
    let week_start = week_end - Duration::days(total_days - 1);

    let labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    for row in 0..7 {
        let mut spans: Vec<Span> = Vec::with_capacity(weeks as usize + 2);
        spans.push(Span::styled(
            format!(" {}  ", labels[row]),
            Style::default().fg(Color::DarkGray),
        ));
        for col in 0..weeks as usize {
            let date = week_start + Duration::days((col as i64) * 7 + row as i64);
            spans.push(binary_cell(habit, date, today, state));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    lines.push(binary_legend(is_quit));

    f.render_widget(Paragraph::new(lines), inner);
}

fn binary_cell(
    habit: &Habit,
    date: NaiveDate,
    today: NaiveDate,
    state: &DetailState,
) -> Span<'static> {
    let is_cursor = state.edit_mode && date == state.cursor;
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });

    if date > today {
        // future
        if is_cursor {
            return Span::styled(
                "[]",
                Style::default().fg(COL_ACCENT).add_modifier(Modifier::BOLD),
            );
        }
        return Span::styled("  ", Style::default());
    }
    if date < habit.created_at {
        let s = Style::default().fg(Color::Rgb(40, 42, 50));
        let s = if is_cursor { s.add_modifier(Modifier::REVERSED) } else { s };
        return Span::styled(CELL, s);
    }

    let hit = match &habit.kind {
        HabitKind::Build => habit.completions.contains(&date),
        HabitKind::Quit { failures } => failures.contains(&date),
    };

    let (glyph, color) = if is_quit {
        if hit {
            (CELL_HALF, COL_DANGER)
        } else {
            // for a Quit habit, "missed" = clean = good
            (CELL, Color::Rgb(80, 140, 100))
        }
    } else if hit {
        (CELL, Color::Rgb(150, 220, 130))
    } else {
        (CELL, Color::Rgb(60, 64, 74))
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
            Span::styled("Legend: ", Style::default().fg(Color::DarkGray)),
            Span::styled(CELL, Style::default().fg(Color::Rgb(80, 140, 100))),
            Span::styled(" clean   ", Style::default().fg(Color::DarkGray)),
            Span::styled(CELL_HALF, Style::default().fg(COL_DANGER)),
            Span::styled(" failure   ", Style::default().fg(Color::DarkGray)),
            Span::styled(CELL, Style::default().fg(Color::Rgb(40, 42, 50))),
            Span::styled(" before created", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::raw("    "),
            Span::styled("Legend: ", Style::default().fg(Color::DarkGray)),
            Span::styled(CELL, Style::default().fg(Color::Rgb(150, 220, 130))),
            Span::styled(" done   ", Style::default().fg(Color::DarkGray)),
            Span::styled(CELL, Style::default().fg(Color::Rgb(60, 64, 74))),
            Span::styled(" missed   ", Style::default().fg(Color::DarkGray)),
            Span::styled(CELL, Style::default().fg(Color::Rgb(40, 42, 50))),
            Span::styled(" before created", Style::default().fg(Color::DarkGray)),
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

    let (glyph, glyph_color) = streak_glyph(current, is_quit);
    let border_color = motivation_border_color(current);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
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
                .bg(Color::Rgb(180, 180, 200))
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " BUILD ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(140, 200, 160))
                .add_modifier(Modifier::BOLD),
        )
    };

    let streak_label = if is_quit {
        format!("{} days clean", current)
    } else {
        format!("{} day streak", current)
    };

    let line = Line::from(vec![
        kind_tag,
        Span::raw("  "),
        Span::styled(
            habit.name.clone(),
            Style::default().fg(COL_HEADER).add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ", Style::default()),
        Span::styled(format_frequency(habit.frequency), Style::default().fg(Color::DarkGray)),
        Span::styled("   ·   ", Style::default().fg(COL_DIM)),
        Span::styled(
            format!("{} ", glyph),
            Style::default().fg(glyph_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            streak_label,
            Style::default().fg(glyph_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ·   ", Style::default().fg(COL_DIM)),
        Span::styled("longest ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", longest),
            Style::default().fg(Color::Rgb(200, 200, 220)).add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ·   ", Style::default().fg(COL_DIM)),
        Span::styled(
            if is_quit { "failures " } else { "total " },
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{}", total),
            Style::default().fg(Color::Rgb(200, 200, 220)).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line), layout[0]);
}

fn motivation_border_color(streak: u32) -> Color {
    match streak {
        0..=2 => COL_DIM,
        3..=6 => Color::Rgb(140, 180, 130),
        7..=13 => Color::Rgb(200, 180, 110),
        14..=29 => Color::Rgb(230, 160, 100),
        30..=99 => Color::Rgb(240, 130, 100),
        _ => Color::Rgb(245, 110, 140),
    }
}

fn render_motivation_line(f: &mut Frame, habit: &Habit, today: NaiveDate, area: Rect) {
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });
    let streak = habit.current_streak(today);
    let (msg, color) = affirmation(streak, is_quit);
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            msg,
            Style::default().fg(color).add_modifier(Modifier::ITALIC),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn affirmation(streak: u32, is_quit: bool) -> (String, Color) {
    let color = motivation_border_color(streak);
    let msg = if is_quit {
        match streak {
            0 => "A reset, not a defeat — start fresh tomorrow.",
            1..=2 => "One day at a time. The hardest part is starting.",
            3..=6 => "A few days clean — the urge gets quieter from here.",
            7..=13 => "A full week clean. Real progress.",
            14..=29 => "Two weeks. The new normal is taking shape.",
            30..=99 => "A month clean. This is who you are now.",
            _ => "A hundred days. Quiet, steady, hard-won.",
        }
    } else {
        match streak {
            0 => "Today is a good day to begin.",
            1..=2 => "First step taken. Keep it small, keep it kind.",
            3..=6 => "Three days in — the rhythm is forming.",
            7..=13 => "A week deep. Consistency over intensity.",
            14..=29 => "Two weeks. This is becoming a habit, not a chore.",
            30..=99 => "A full month. You don't need motivation any more — you have momentum.",
            _ => "A hundred days. This is mastery, quietly accumulating.",
        }
    };
    (msg.to_string(), color)
}

// ---------- Global heatmap (across all habits) ----------

fn render_global_heatmap(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(2), // affirmation
            Constraint::Min(10),   // heatmap
            Constraint::Length(1), // footer
        ])
        .split(area);

    render_global_header(f, &app.store, app.today, chunks[0]);
    render_global_summary(f, &app.store, app.today, chunks[1]);
    render_global_grid(f, &app.store, app.today, chunks[2]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("  [esc/q/g/⏎] ", Style::default().fg(COL_NAV).add_modifier(Modifier::BOLD)),
        Span::styled("back to list", Style::default().fg(Color::Rgb(180, 180, 195))),
    ]));
    f.render_widget(footer, chunks[3]);
}

fn render_global_header(f: &mut Frame, store: &HabitStore, today: NaiveDate, area: Rect) {
    let weeks: i64 = 26;
    let start = today - Duration::days(weeks * 7 - 1);
    let mut total_completions: u64 = 0;
    let mut active_days = std::collections::BTreeSet::<NaiveDate>::new();
    for h in &store.habits {
        if !matches!(h.kind, HabitKind::Build) {
            continue;
        }
        for d in h.completions.range(start..=today) {
            total_completions += 1;
            active_days.insert(*d);
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COL_ACCENT));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let line = Line::from(vec![
        Span::styled(
            " GLOBAL ",
            Style::default()
                .fg(Color::Black)
                .bg(COL_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            "Activity across all habits",
            Style::default().fg(COL_HEADER).add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ·   ", Style::default().fg(COL_DIM)),
        Span::styled(
            format!("{} weeks", weeks),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("   ·   ", Style::default().fg(COL_DIM)),
        Span::styled(
            format!("{} completions", total_completions),
            Style::default()
                .fg(Color::Rgb(200, 200, 220))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ·   ", Style::default().fg(COL_DIM)),
        Span::styled(
            format!("{} active days", active_days.len()),
            Style::default()
                .fg(Color::Rgb(200, 200, 220))
                .add_modifier(Modifier::BOLD),
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

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{} build", build_count),
            Style::default().fg(Color::Rgb(140, 200, 160)),
        ),
        Span::styled("   ·   ", Style::default().fg(COL_DIM)),
        Span::styled(
            format!("{} quit", quit_count),
            Style::default().fg(Color::Rgb(200, 180, 220)),
        ),
        Span::styled("   ·   ", Style::default().fg(COL_DIM)),
        Span::styled(
            format!("{} completed today", done_today),
            Style::default().fg(COL_ACCENT).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_global_grid(f: &mut Frame, store: &HabitStore, today: NaiveDate, area: Rect) {
    let weeks: i64 = 26;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COL_DIM))
        .title(Span::styled(
            format!(" Last {} weeks · combined activity ", weeks),
            Style::default().fg(COL_HEADER).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Aggregate Build completions per date.
    let dow = today.weekday().num_days_from_monday() as i64;
    let week_end = today + Duration::days(6 - dow);
    let total_days = weeks * 7;
    let week_start = week_end - Duration::days(total_days - 1);

    let mut counts: BTreeMap<NaiveDate, u32> = BTreeMap::new();
    for h in &store.habits {
        if !matches!(h.kind, HabitKind::Build) {
            continue;
        }
        for d in h.completions.range(week_start..=today) {
            *counts.entry(*d).or_insert(0) += 1;
        }
    }
    let max_count = counts.values().copied().max().unwrap_or(0);

    let labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    for row in 0..7 {
        let mut spans: Vec<Span> = Vec::with_capacity(weeks as usize + 2);
        spans.push(Span::styled(
            format!(" {}  ", labels[row]),
            Style::default().fg(Color::DarkGray),
        ));
        for col in 0..weeks as usize {
            let date = week_start + Duration::days((col as i64) * 7 + row as i64);
            spans.push(global_cell(date, today, &counts, max_count));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    lines.push(global_legend());

    f.render_widget(Paragraph::new(lines), inner);
}

fn global_cell(
    date: NaiveDate,
    today: NaiveDate,
    counts: &BTreeMap<NaiveDate, u32>,
    max_count: u32,
) -> Span<'static> {
    if date > today {
        return Span::styled("  ", Style::default());
    }
    let n = counts.get(&date).copied().unwrap_or(0);
    let level = if max_count == 0 || n == 0 {
        0
    } else {
        // Map to 1..=4.
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
        0 => Color::Rgb(38, 42, 52),
        1 => Color::Rgb(80, 110, 150),
        2 => Color::Rgb(120, 160, 200),
        3 => Color::Rgb(180, 200, 240),
        _ => Color::Rgb(245, 200, 90),
    }
}

fn global_legend() -> Line<'static> {
    Line::from(vec![
        Span::raw("    "),
        Span::styled("Less ", Style::default().fg(Color::DarkGray)),
        Span::styled(CELL, Style::default().fg(global_palette(0))),
        Span::raw(" "),
        Span::styled(CELL, Style::default().fg(global_palette(1))),
        Span::raw(" "),
        Span::styled(CELL, Style::default().fg(global_palette(2))),
        Span::raw(" "),
        Span::styled(CELL, Style::default().fg(global_palette(3))),
        Span::raw(" "),
        Span::styled(CELL, Style::default().fg(global_palette(4))),
        Span::styled(" More", Style::default().fg(Color::DarkGray)),
    ])
}
