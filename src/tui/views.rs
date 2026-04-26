use chrono::{Datelike, Duration, NaiveDate};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::data::{Frequency, Habit, HabitKind};
use crate::tui::app::{
    AddForm, App, EditForm, FormField, FrequencyChoice, KindChoice, Screen,
};

const HEATMAP_WEEKS: i64 = 13;
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
        Screen::Detail { habit_id } => {
            let id = *habit_id;
            render_detail(f, app, id);
        }
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
            "habitui",
            Style::default()
                .fg(COL_HEADER)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  ·  ",
            Style::default().fg(COL_DIM),
        ),
        Span::styled(
            format!("{}", app.today.format("%a %Y-%m-%d")),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("  ·  {} habit{}", app.store.habits.len(),
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
        Frequency::Weekly => "Weekly".to_string(),
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
    extend_spans(&mut spans, keybinding("⏎", "graph", COL_NAV));
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
    let height = if form.freq_choice.needs_numeric() { 17 } else { 15 };
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

// ---------- Edit form ----------

fn render_edit_form(f: &mut Frame, form: &EditForm) {
    let height = if form.freq_choice.needs_numeric() { 17 } else { 15 };
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
        (FrequencyChoice::Weekly, "[Weekly]"),
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

// ---------- Detail / heatmap ----------

fn render_detail(f: &mut Frame, app: &App, habit_id: u64) {
    let area = f.area();
    let habit = match app.store.habit(habit_id) {
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
            Constraint::Min(10),   // heatmap
            Constraint::Length(1), // footer
        ])
        .split(area);

    render_detail_header(f, habit, app.today, chunks[0]);
    render_motivation_line(f, habit, app.today, chunks[1]);
    render_heatmap(f, habit, app.today, chunks[2]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("[esc/q] ", Style::default().fg(COL_NAV).add_modifier(Modifier::BOLD)),
        Span::styled("back to list", Style::default().fg(Color::Rgb(180, 180, 195))),
        Span::styled("   ·   ", Style::default().fg(COL_DIM)),
        Span::styled("[e] ", Style::default().fg(COL_MUT).add_modifier(Modifier::BOLD)),
        Span::styled("edit (from list)", Style::default().fg(Color::Rgb(180, 180, 195))),
    ]));
    f.render_widget(footer, chunks[3]);
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

fn render_heatmap(f: &mut Frame, habit: &Habit, today: NaiveDate, area: Rect) {
    let is_quit = matches!(habit.kind, HabitKind::Quit { .. });
    let title = if is_quit {
        format!(" Last {} weeks · failures shown ", HEATMAP_WEEKS)
    } else {
        format!(" Last {} weeks · activity heatmap ", HEATMAP_WEEKS)
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

    // Anchor: end of current ISO week (Sunday).
    let dow = today.weekday().num_days_from_monday() as i64;
    let week_end = today + Duration::days(6 - dow);
    let total_days = HEATMAP_WEEKS * 7;
    let week_start = week_end - Duration::days(total_days - 1);

    // Build grid: rows=weekday (Mon..Sun), cols=week index.
    let mut date_grid: Vec<Vec<NaiveDate>> = vec![vec![week_start; HEATMAP_WEEKS as usize]; 7];
    for col in 0..HEATMAP_WEEKS {
        let col_start = week_start + Duration::days(col * 7);
        for row in 0..7 {
            date_grid[row as usize][col as usize] = col_start + Duration::days(row as i64);
        }
    }

    // Per-day "intensity": 0..=4 for Build, 0|hit for Quit (hit = failure).
    // For Build daily/weekly/everyN: any completion → level 4 (binary).
    // For NTimesPerWeek(n): each completion within its ISO week is also a hit;
    //   we color the cell by the week's count/n ratio if the day is completed
    //   (so a fuller week shows brighter even with the same dot).
    let labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let mut lines: Vec<Line> = Vec::with_capacity(9);

    // Row of week-month markers (just an empty padding row keeps breathing room).
    lines.push(Line::from(""));

    for row in 0..7 {
        let mut spans: Vec<Span> = Vec::with_capacity(HEATMAP_WEEKS as usize + 2);
        spans.push(Span::styled(
            format!(" {}  ", labels[row]),
            Style::default().fg(Color::DarkGray),
        ));
        for col in 0..HEATMAP_WEEKS as usize {
            let date = date_grid[row][col];
            let span = render_cell(habit, date, today, is_quit);
            spans.push(span);
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
    }

    // Breathing room + legend.
    lines.push(Line::from(""));
    lines.push(legend_line(is_quit));

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_cell(habit: &Habit, date: NaiveDate, today: NaiveDate, is_quit: bool) -> Span<'static> {
    if date > today {
        return Span::styled(
            "  ",
            Style::default().fg(Color::Black),
        );
    }
    if date < habit.created_at {
        return Span::styled(CELL, Style::default().fg(Color::Rgb(28, 30, 38)));
    }

    if is_quit {
        let failure = match &habit.kind {
            HabitKind::Quit { failures } => failures.contains(&date),
            _ => false,
        };
        return if failure {
            Span::styled(CELL_HALF, Style::default().fg(COL_DANGER))
        } else {
            Span::styled(CELL, Style::default().fg(Color::Rgb(60, 100, 70)))
        };
    }

    let done = habit.completions.contains(&date);
    let level: u8 = match habit.frequency {
        Frequency::NTimesPerWeek(n) if done => {
            let monday = iso_week_monday(date);
            let sunday = monday + Duration::days(6);
            let count = habit.completions.range(monday..=sunday).count() as u32;
            // 1.. n: 1→2, mid→3, ≥n→4.
            let n = n.max(1);
            if count >= n {
                4
            } else if count * 2 >= n {
                3
            } else {
                2
            }
        }
        _ if done => 4,
        _ => 0,
    };
    let color = build_palette(level);
    Span::styled(CELL, Style::default().fg(color))
}

fn build_palette(level: u8) -> Color {
    match level {
        0 => Color::Rgb(38, 42, 52),     // empty (almost background)
        1 => Color::Rgb(60, 100, 70),
        2 => Color::Rgb(80, 140, 90),
        3 => Color::Rgb(110, 180, 110),
        _ => Color::Rgb(150, 220, 130),  // brightest
    }
}

fn legend_line(is_quit: bool) -> Line<'static> {
    if is_quit {
        Line::from(vec![
            Span::raw("    "),
            Span::styled("Legend: ", Style::default().fg(Color::DarkGray)),
            Span::styled(CELL, Style::default().fg(Color::Rgb(60, 100, 70))),
            Span::styled(" clean   ", Style::default().fg(Color::DarkGray)),
            Span::styled(CELL_HALF, Style::default().fg(COL_DANGER)),
            Span::styled(" failure logged   ", Style::default().fg(Color::DarkGray)),
            Span::styled(CELL, Style::default().fg(Color::Rgb(28, 30, 38))),
            Span::styled(" before created", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::raw("    "),
            Span::styled("Less ", Style::default().fg(Color::DarkGray)),
            Span::styled(CELL, Style::default().fg(build_palette(0))),
            Span::raw(" "),
            Span::styled(CELL, Style::default().fg(build_palette(1))),
            Span::raw(" "),
            Span::styled(CELL, Style::default().fg(build_palette(2))),
            Span::raw(" "),
            Span::styled(CELL, Style::default().fg(build_palette(3))),
            Span::raw(" "),
            Span::styled(CELL, Style::default().fg(build_palette(4))),
            Span::styled(" More", Style::default().fg(Color::DarkGray)),
        ])
    }
}
