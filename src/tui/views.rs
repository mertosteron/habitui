use chrono::{Duration, NaiveDate, Datelike};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::data::{Frequency, Habit};
use crate::tui::app::{AddField, AddForm, App, FrequencyChoice, Screen};

const HEATMAP_WEEKS: i64 = 12;
const CELL: &str = "\u{2588}\u{2588}"; // "██"

pub fn render(f: &mut Frame, app: &mut App) {
    match &app.screen {
        Screen::List => render_list(f, app),
        Screen::AddHabit(_) => {
            // Render list as backdrop, then form on top.
            render_list(f, app);
            if let Screen::AddHabit(form) = &app.screen {
                render_add_form(f, form);
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
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "habit-tracker",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{}", app.today.format("%a %Y-%m-%d")),
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
            "  (no habits yet — press 'a' to add one)",
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
        .block(Block::default().borders(Borders::ALL).title(" Habits "))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
    f.render_stateful_widget(list, chunks[1], &mut list_state);

    let status = match &app.status {
        Some(msg) => msg.clone(),
        None => "j/k move  space toggle  a add  d delete  g graph  q quit".to_string(),
    };
    let footer = Paragraph::new(Span::styled(
        status,
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(footer, chunks[2]);
}

fn habit_row(h: &Habit, today: NaiveDate) -> Line<'static> {
    let name = pad(&h.name, 30);
    let freq = pad(&format_frequency(h.frequency), 16);
    let streak = h.current_streak(today);
    let streak_str = if streak > 0 {
        format!("\u{1F525}{:<7}", streak) // 🔥<n>
    } else {
        format!("*{:<7}", streak)
    };
    let done_today = h.completions.contains(&today);
    let mark = if done_today { "\u{2713}" } else { "\u{00B7}" };
    let mark_color = if done_today { Color::Green } else { Color::DarkGray };

    Line::from(vec![
        Span::raw("  "),
        Span::raw(name),
        Span::raw(freq),
        Span::styled(streak_str, Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled(mark.to_string(), Style::default().fg(mark_color)),
    ])
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
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

fn render_add_form(f: &mut Frame, form: &AddForm) {
    let area = centered_rect(60, 13, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Add habit ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // name label
            Constraint::Length(1), // name input
            Constraint::Length(1), // gap
            Constraint::Length(1), // freq label
            Constraint::Length(1), // freq picker
            Constraint::Length(1), // gap
            Constraint::Length(1), // every-n label/input (when relevant)
            Constraint::Min(1),    // help / error
        ])
        .split(inner);

    let label = |text: &'static str, focused: bool| {
        let style = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        Paragraph::new(Span::styled(text, style))
    };

    f.render_widget(label("Name:", form.field == AddField::Name), layout[0]);

    let name_display = if form.name.is_empty() {
        Span::styled("<type a name>", Style::default().fg(Color::DarkGray))
    } else {
        Span::raw(form.name.clone())
    };
    let name_style = if form.field == AddField::Name {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::raw(" "), name_display])).style(name_style),
        layout[1],
    );

    f.render_widget(
        label("Frequency: (Tab to change focus, Space/← →  to cycle)", form.field == AddField::Frequency),
        layout[3],
    );

    let mut spans: Vec<Span> = Vec::new();
    for choice in [
        FrequencyChoice::Daily,
        FrequencyChoice::Weekly,
        FrequencyChoice::EveryNDays,
    ] {
        let label_str = match choice {
            FrequencyChoice::Daily => "[Daily]",
            FrequencyChoice::Weekly => "[Weekly]",
            FrequencyChoice::EveryNDays => "[Every N days]",
        };
        let mut style = Style::default();
        if choice == form.freq_choice {
            style = style.fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
        } else {
            style = style.fg(Color::DarkGray);
        }
        spans.push(Span::styled(label_str, style));
        spans.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), layout[4]);

    if form.freq_choice == FrequencyChoice::EveryNDays {
        let focused = form.field == AddField::EveryNValue;
        let style = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let line = Line::from(vec![
            Span::styled("N = ", style),
            Span::styled(
                format!("{}", form.every_n),
                if focused {
                    Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
            Span::styled("  (digits to edit, Backspace to clear)", Style::default().fg(Color::DarkGray)),
        ]);
        f.render_widget(Paragraph::new(line), layout[6]);
    }

    let help_text = match &form.error {
        Some(e) => Line::from(Span::styled(
            e.clone(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        None => Line::from(Span::styled(
            "Enter saves  Esc cancels",
            Style::default().fg(Color::DarkGray),
        )),
    };
    f.render_widget(Paragraph::new(help_text), layout[7]);
}

fn render_confirm_delete(f: &mut Frame, app: &App, habit_id: u64) {
    let name = app
        .store
        .habit(habit_id)
        .map(|h| h.name.clone())
        .unwrap_or_else(|| "<missing>".to_string());

    let area = centered_rect(50, 5, f.area());
    f.render_widget(Clear, area);

    let block = Block::default().borders(Borders::ALL).title(" Confirm delete ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("Delete \""),
            Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("\" ? (y/n)"),
        ]),
    ];
    let p = Paragraph::new(lines).alignment(Alignment::Center);
    f.render_widget(p, inner);
}

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
            Constraint::Length(1), // title
            Constraint::Length(4), // metadata block
            Constraint::Min(10),   // heatmap
            Constraint::Length(1), // footer
        ])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            habit.name.clone(),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format_frequency(habit.frequency),
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    f.render_widget(title, chunks[0]);

    let current = habit.current_streak(app.today);
    let longest = habit.longest_streak();
    let total = habit.completions.len();
    let meta = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Current streak: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", current),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled("Longest streak: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", longest),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled("Total completions: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", total), Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![Span::styled(
            format!("Created {}", habit.created_at.format("%Y-%m-%d")),
            Style::default().fg(Color::DarkGray),
        )]),
    ])
    .block(Block::default().borders(Borders::NONE));
    f.render_widget(meta, chunks[1]);

    render_heatmap(f, habit, app.today, chunks[2]);

    f.render_widget(
        Paragraph::new(Span::styled(
            "Esc/q/Enter to return to list",
            Style::default().fg(Color::DarkGray),
        )),
        chunks[3],
    );
}

fn render_heatmap(f: &mut Frame, habit: &Habit, today: NaiveDate, area: Rect) {
    // 12 columns of weeks; each column has 7 days (Mon..Sun).
    // The last column ends at `today`'s week. We anchor by walking back from
    // Sunday-of-this-week so columns align to whole weeks.
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Last ~{} weeks ", HEATMAP_WEEKS));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // End-of-current-week (Sunday). chrono's Weekday::num_days_from_monday:
    // Mon=0..Sun=6, so days_until_sunday = 6 - that.
    let dow = today.weekday().num_days_from_monday() as i64;
    let week_end = today + Duration::days(6 - dow);
    let total_days = HEATMAP_WEEKS * 7;
    let week_start = week_end - Duration::days(total_days - 1);

    // Build a 7×12 grid.  rows = days of week (Mon..Sun), cols = week index.
    let mut grid: Vec<Vec<bool>> = vec![vec![false; HEATMAP_WEEKS as usize]; 7];
    let mut date_grid: Vec<Vec<NaiveDate>> = vec![vec![week_start; HEATMAP_WEEKS as usize]; 7];
    for col in 0..HEATMAP_WEEKS {
        let col_start = week_start + Duration::days(col * 7);
        for row in 0..7 {
            let d = col_start + Duration::days(row as i64);
            date_grid[row as usize][col as usize] = d;
            // Don't mark future days as "empty" specially; they just stay false.
            if d <= today && habit.completions.contains(&d) {
                grid[row as usize][col as usize] = true;
            }
        }
    }

    // Render rows. Each row begins with a 3-char weekday label.
    let labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let mut lines: Vec<Line> = Vec::with_capacity(8);
    for row in 0..7 {
        let mut spans: Vec<Span> = Vec::with_capacity(HEATMAP_WEEKS as usize + 1);
        spans.push(Span::styled(
            format!("{}  ", labels[row]),
            Style::default().fg(Color::DarkGray),
        ));
        for col in 0..HEATMAP_WEEKS as usize {
            let date = date_grid[row][col];
            let style = if date > today {
                // Future cell — render blank space so it doesn't read as a missed day.
                Style::default().fg(Color::Black)
            } else if grid[row][col] {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let glyph = if date > today { "  " } else { CELL };
            spans.push(Span::styled(glyph, style));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
    }
    // Legend.
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Legend: ", Style::default().fg(Color::DarkGray)),
        Span::styled(CELL, Style::default().fg(Color::Green)),
        Span::raw(" done   "),
        Span::styled(CELL, Style::default().fg(Color::DarkGray)),
        Span::raw(" missed"),
    ]));

    f.render_widget(Paragraph::new(lines), inner);
}
