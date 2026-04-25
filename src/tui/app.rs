use std::io::{self, Stdout};

use chrono::{Local, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::data::{Frequency, HabitStore};
use crate::tui::events::next_key;
use crate::tui::views;

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;

/// Top-level screen the app is currently showing.
pub enum Screen {
    List,
    AddHabit(AddForm),
    Detail { habit_id: u64 },
    ConfirmDelete { habit_id: u64 },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AddField {
    Name,
    Frequency,
    EveryNValue,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FrequencyChoice {
    Daily,
    Weekly,
    EveryNDays,
}

pub struct AddForm {
    pub name: String,
    pub field: AddField,
    pub freq_choice: FrequencyChoice,
    pub every_n: u32,
    pub every_n_dirty: bool,
    pub error: Option<String>,
}

impl AddForm {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            field: AddField::Name,
            freq_choice: FrequencyChoice::Daily,
            every_n: 2,
            every_n_dirty: false,
            error: None,
        }
    }

    pub fn frequency(&self) -> Frequency {
        match self.freq_choice {
            FrequencyChoice::Daily => Frequency::Daily,
            FrequencyChoice::Weekly => Frequency::Weekly,
            FrequencyChoice::EveryNDays => Frequency::EveryNDays(self.every_n.max(1)),
        }
    }

    pub fn cycle_field_forward(&mut self) {
        self.field = match self.field {
            AddField::Name => AddField::Frequency,
            AddField::Frequency => {
                if self.freq_choice == FrequencyChoice::EveryNDays {
                    AddField::EveryNValue
                } else {
                    AddField::Name
                }
            }
            AddField::EveryNValue => AddField::Name,
        };
    }

    pub fn cycle_freq_choice(&mut self) {
        self.freq_choice = match self.freq_choice {
            FrequencyChoice::Daily => FrequencyChoice::Weekly,
            FrequencyChoice::Weekly => FrequencyChoice::EveryNDays,
            FrequencyChoice::EveryNDays => FrequencyChoice::Daily,
        };
    }
}

pub struct App {
    pub store: HabitStore,
    pub screen: Screen,
    pub selected: usize,
    pub today: NaiveDate,
    pub should_quit: bool,
    pub status: Option<String>,
}

impl App {
    pub fn new(store: HabitStore) -> Self {
        Self {
            store,
            screen: Screen::List,
            selected: 0,
            today: Local::now().date_naive(),
            should_quit: false,
            status: None,
        }
    }

    fn clamp_selection(&mut self) {
        let len = self.store.habits.len();
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    fn current_habit_id(&self) -> Option<u64> {
        self.store.habits.get(self.selected).map(|h| h.id)
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // Global Ctrl-C as a safety quit, regardless of screen.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        // Status messages live for one keypress so the keybinding hint comes back.
        self.status = None;

        // Take ownership of the screen so we can mutate inner state freely.
        let screen = std::mem::replace(&mut self.screen, Screen::List);
        self.screen = match screen {
            Screen::List => self.handle_list_key(key),
            Screen::AddHabit(form) => self.handle_add_key(key, form),
            Screen::Detail { habit_id } => self.handle_detail_key(key, habit_id),
            Screen::ConfirmDelete { habit_id } => self.handle_confirm_key(key, habit_id),
        };
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Screen {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.store.habits.is_empty() {
                    self.selected = (self.selected + 1).min(self.store.habits.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Char(' ') => {
                if let Some(id) = self.current_habit_id() {
                    let _ = self.store.toggle_completion(id, self.today);
                }
            }
            KeyCode::Char('a') => {
                return Screen::AddHabit(AddForm::new());
            }
            KeyCode::Char('d') => {
                if let Some(id) = self.current_habit_id() {
                    return Screen::ConfirmDelete { habit_id: id };
                }
            }
            KeyCode::Enter => {
                if let Some(id) = self.current_habit_id() {
                    return Screen::Detail { habit_id: id };
                }
            }
            _ => {}
        }
        Screen::List
    }

    fn handle_add_key(&mut self, key: KeyEvent, mut form: AddForm) -> Screen {
        match key.code {
            KeyCode::Esc => {
                return Screen::List;
            }
            KeyCode::Tab => form.cycle_field_forward(),
            KeyCode::Enter => {
                let name = form.name.trim().to_string();
                if name.is_empty() {
                    form.error = Some("Name cannot be empty.".to_string());
                    return Screen::AddHabit(form);
                }
                let freq = form.frequency();
                self.store.add_habit(name, freq, self.today);
                self.selected = self.store.habits.len().saturating_sub(1);
                self.status = Some("Added habit.".to_string());
                return Screen::List;
            }
            _ => match form.field {
                AddField::Name => match key.code {
                    KeyCode::Char(c) => form.name.push(c),
                    KeyCode::Backspace => {
                        form.name.pop();
                    }
                    _ => {}
                },
                AddField::Frequency => match key.code {
                    KeyCode::Char(' ')
                    | KeyCode::Right
                    | KeyCode::Char('l')
                    | KeyCode::Char('j')
                    | KeyCode::Down => form.cycle_freq_choice(),
                    KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('k') | KeyCode::Up => {
                        // Cycle backward = cycle three times forward.
                        form.cycle_freq_choice();
                        form.cycle_freq_choice();
                    }
                    _ => {}
                },
                AddField::EveryNValue => match key.code {
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        let digit = c.to_digit(10).unwrap();
                        let base = if form.every_n_dirty { form.every_n } else { 0 };
                        let next = base.saturating_mul(10).saturating_add(digit);
                        form.every_n = next.min(999).max(1);
                        form.every_n_dirty = true;
                    }
                    KeyCode::Backspace => {
                        form.every_n = (form.every_n / 10).max(1);
                        form.every_n_dirty = true;
                    }
                    _ => {}
                },
            },
        }
        Screen::AddHabit(form)
    }

    fn handle_detail_key(&mut self, key: KeyEvent, habit_id: u64) -> Screen {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => Screen::List,
            _ => Screen::Detail { habit_id },
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent, habit_id: u64) -> Screen {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if self.store.remove_habit(habit_id) {
                    self.clamp_selection();
                    self.status = Some("Deleted habit.".to_string());
                }
                Screen::List
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Screen::List,
            _ => Screen::ConfirmDelete { habit_id },
        }
    }
}

/// RAII guard: enables raw mode + alt screen on construction, restores on drop.
/// Drop runs even on panic, so the parent shell stays usable.
struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(e) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(e);
        }
        Ok(Self { active: true })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
        }
    }
}

pub fn run_app(store: &mut HabitStore) -> io::Result<()> {
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal: Terminal<CrosstermBackend<Stdout>> = Terminal::new(backend)?;

    // Move the store into App so the borrow checker leaves us alone, then
    // hand it back at the end. This keeps `main.rs` straightforward.
    let owned_store = std::mem::replace(store, HabitStore::new());
    let mut app = App::new(owned_store);

    let result = event_loop(&mut terminal, &mut app);

    *store = std::mem::replace(&mut app.store, HabitStore::new());
    result
}

fn event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    while !app.should_quit {
        terminal.draw(|f| {
            let area = f.area();
            if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
                views::render_resize_notice(f, area);
                return;
            }
            views::render(f, app);
        })?;

        if let Some(key) = next_key()? {
            app.handle_key(key);
        }
    }
    Ok(())
}
