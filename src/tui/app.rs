use std::io::{self, Stdout};

use chrono::{Datelike, Duration, Local, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::data::{Frequency, HabitKind, HabitStore};
use crate::tui::events::next_key;
use crate::tui::views;

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;

pub enum Screen {
    List,
    AddHabit(AddForm),
    EditHabit(EditForm),
    Detail(DetailState),
    GlobalHeatmap,
    ConfirmDelete { habit_id: u64 },
}

/// Per-habit detail-view state. `edit_mode` toggles the inline cell editor;
/// `cursor` is the selected calendar day when editing.
pub struct DetailState {
    pub habit_id: u64,
    pub edit_mode: bool,
    pub cursor: NaiveDate,
}

impl DetailState {
    pub fn new(habit_id: u64, today: NaiveDate) -> Self {
        Self {
            habit_id,
            edit_mode: false,
            cursor: today,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    Name,
    Kind,
    Frequency,
    NumericValue,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FrequencyChoice {
    Daily,
    EveryNDays,
    NTimesPerWeek,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KindChoice {
    Build,
    Quit,
}

impl FrequencyChoice {
    pub fn needs_numeric(self) -> bool {
        matches!(
            self,
            FrequencyChoice::EveryNDays | FrequencyChoice::NTimesPerWeek
        )
    }

    pub fn from_frequency(f: Frequency) -> Self {
        match f {
            Frequency::Daily => FrequencyChoice::Daily,
            Frequency::EveryNDays(_) => FrequencyChoice::EveryNDays,
            Frequency::NTimesPerWeek(_) => FrequencyChoice::NTimesPerWeek,
        }
    }
}

pub struct AddForm {
    pub name: String,
    pub field: FormField,
    pub kind_choice: KindChoice,
    pub freq_choice: FrequencyChoice,
    pub numeric_buf: String,
    pub error: Option<String>,
}

pub struct EditForm {
    pub habit_id: u64,
    pub name: String,
    pub field: FormField,
    pub freq_choice: FrequencyChoice,
    pub numeric_buf: String,
    pub kind_label: &'static str,
    pub is_quit: bool,
    pub error: Option<String>,
}

impl AddForm {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            field: FormField::Name,
            kind_choice: KindChoice::Build,
            freq_choice: FrequencyChoice::Daily,
            numeric_buf: "2".to_string(),
            error: None,
        }
    }

    pub fn cycle_field_forward(&mut self) {
        let is_quit = matches!(self.kind_choice, KindChoice::Quit);
        self.field = match self.field {
            FormField::Name => FormField::Kind,
            FormField::Kind => {
                if is_quit {
                    FormField::Name
                } else {
                    FormField::Frequency
                }
            }
            FormField::Frequency => {
                if self.freq_choice.needs_numeric() {
                    FormField::NumericValue
                } else {
                    FormField::Name
                }
            }
            FormField::NumericValue => FormField::Name,
        };
    }

    pub fn cycle_kind(&mut self) {
        self.kind_choice = match self.kind_choice {
            KindChoice::Build => KindChoice::Quit,
            KindChoice::Quit => KindChoice::Build,
        };
        if matches!(self.kind_choice, KindChoice::Quit) {
            // Quit habits are tracked daily; force the choice and clear stale focus.
            self.freq_choice = FrequencyChoice::Daily;
            if matches!(self.field, FormField::Frequency | FormField::NumericValue) {
                self.field = FormField::Kind;
            }
        }
    }

    pub fn cycle_freq_forward(&mut self) {
        self.freq_choice = cycle_freq(self.freq_choice, true);
    }

    pub fn cycle_freq_backward(&mut self) {
        self.freq_choice = cycle_freq(self.freq_choice, false);
    }

    pub fn parse_frequency(&self) -> Result<Frequency, String> {
        if matches!(self.kind_choice, KindChoice::Quit) {
            return Ok(Frequency::Daily);
        }
        parse_frequency_from(self.freq_choice, &self.numeric_buf)
    }

    pub fn parse_kind(&self) -> HabitKind {
        match self.kind_choice {
            KindChoice::Build => HabitKind::Build,
            KindChoice::Quit => HabitKind::Quit {
                failures: Default::default(),
            },
        }
    }
}

impl EditForm {
    pub fn from_habit(id: u64, name: &str, freq: Frequency, kind: &HabitKind) -> Self {
        let is_quit = matches!(kind, HabitKind::Quit { .. });
        // Quit habits ignore frequency in their behavior; pin the picker to Daily
        // so the user can't see or change it. Build habits keep their stored value.
        let freq_choice = if is_quit {
            FrequencyChoice::Daily
        } else {
            FrequencyChoice::from_frequency(freq)
        };
        let numeric_buf = match freq {
            Frequency::EveryNDays(n) | Frequency::NTimesPerWeek(n) if !is_quit => n.to_string(),
            _ => String::new(),
        };
        let kind_label = if is_quit { "Quit habit" } else { "Build habit" };
        Self {
            habit_id: id,
            name: name.to_string(),
            field: FormField::Name,
            freq_choice,
            numeric_buf,
            kind_label,
            is_quit,
            error: None,
        }
    }

    pub fn cycle_field_forward(&mut self) {
        if self.is_quit {
            // Only the name field is editable for Quit habits.
            self.field = FormField::Name;
            return;
        }
        self.field = match self.field {
            FormField::Name => FormField::Frequency,
            FormField::Kind | FormField::Frequency => {
                if self.freq_choice.needs_numeric() {
                    FormField::NumericValue
                } else {
                    FormField::Name
                }
            }
            FormField::NumericValue => FormField::Name,
        };
    }

    pub fn cycle_freq_forward(&mut self) {
        self.freq_choice = cycle_freq(self.freq_choice, true);
    }

    pub fn cycle_freq_backward(&mut self) {
        self.freq_choice = cycle_freq(self.freq_choice, false);
    }

    pub fn parse_frequency(&self) -> Result<Frequency, String> {
        if self.is_quit {
            return Ok(Frequency::Daily);
        }
        parse_frequency_from(self.freq_choice, &self.numeric_buf)
    }
}

fn cycle_freq(c: FrequencyChoice, forward: bool) -> FrequencyChoice {
    use FrequencyChoice::*;
    match (c, forward) {
        (Daily, true) => EveryNDays,
        (EveryNDays, true) => NTimesPerWeek,
        (NTimesPerWeek, true) => Daily,
        (Daily, false) => NTimesPerWeek,
        (EveryNDays, false) => Daily,
        (NTimesPerWeek, false) => EveryNDays,
    }
}

fn parse_frequency_from(choice: FrequencyChoice, buf: &str) -> Result<Frequency, String> {
    match choice {
        FrequencyChoice::Daily => Ok(Frequency::Daily),
        FrequencyChoice::EveryNDays => parse_positive_n(buf, "Every-N-days").map(Frequency::EveryNDays),
        FrequencyChoice::NTimesPerWeek => {
            let n = parse_positive_n(buf, "Times-per-week")?;
            if n > 7 {
                return Err("Times per week must be 1..=7.".to_string());
            }
            Ok(Frequency::NTimesPerWeek(n))
        }
    }
}

fn parse_positive_n(buf: &str, label: &str) -> Result<u32, String> {
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} requires a number."));
    }
    match trimmed.parse::<u32>() {
        Ok(n) if (1..=999).contains(&n) => Ok(n),
        Ok(_) => Err(format!("{label} must be between 1 and 999.")),
        Err(_) => Err(format!("{label} must be a whole number.")),
    }
}

pub struct App {
    pub store: HabitStore,
    pub screen: Screen,
    pub selected: usize,
    pub today: NaiveDate,
    pub year: i32,
    pub should_quit: bool,
    pub status: Option<String>,
}

impl App {
    pub fn new(store: HabitStore) -> Self {
        let today = Local::now().date_naive();
        Self {
            store,
            screen: Screen::List,
            selected: 0,
            year: today.year(),
            today,
            should_quit: false,
            status: None,
        }
    }

    fn change_year(&mut self, delta: i32) {
        let new_year = self.year + delta;
        let earliest = self
            .store
            .habits
            .iter()
            .map(|h| h.created_at.year())
            .min()
            .unwrap_or(self.today.year());
        let lo = earliest.min(self.today.year() - 5);
        let hi = self.today.year();
        if new_year < lo || new_year > hi {
            return;
        }
        self.year = new_year;
        self.status = Some(format!("Showing year {}.", self.year));
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
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        self.status = None;

        let screen = std::mem::replace(&mut self.screen, Screen::List);
        self.screen = match screen {
            Screen::List => self.handle_list_key(key),
            Screen::AddHabit(form) => self.handle_add_key(key, form),
            Screen::EditHabit(form) => self.handle_edit_key(key, form),
            Screen::Detail(state) => self.handle_detail_key(key, state),
            Screen::GlobalHeatmap => self.handle_global_heatmap_key(key),
            Screen::ConfirmDelete { habit_id } => self.handle_confirm_key(key, habit_id),
        };
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Screen {
        match key.code {
            KeyCode::Char('q') => {
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
                    if self.store.toggle_completion(id, self.today).is_none() {
                        self.status = Some("Quit habit: press [f] to log a failure.".to_string());
                    }
                }
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                if let Some(id) = self.current_habit_id() {
                    if let Some(habit) = self.store.habit(id) {
                        if matches!(habit.kind, HabitKind::Quit { .. }) {
                            let already = self.store.is_complete_on(id, self.today);
                            if already {
                                let _ = self.store.clear_failure(id, self.today);
                                self.status = Some("Cleared today's failure.".to_string());
                            } else {
                                let _ = self.store.log_failure(id, self.today);
                                self.status = Some("Logged failure for today.".to_string());
                            }
                        } else {
                            self.status = Some("[f] only applies to Quit habits.".to_string());
                        }
                    }
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                return Screen::AddHabit(AddForm::new());
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(id) = self.current_habit_id() {
                    if let Some(h) = self.store.habit(id) {
                        return Screen::EditHabit(EditForm::from_habit(
                            h.id, &h.name, h.frequency, &h.kind,
                        ));
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(id) = self.current_habit_id() {
                    return Screen::ConfirmDelete { habit_id: id };
                }
            }
            KeyCode::Enter => {
                if let Some(id) = self.current_habit_id() {
                    return Screen::Detail(DetailState::new(id, self.today));
                }
            }
            KeyCode::Char('g') | KeyCode::Char('G') => {
                return Screen::GlobalHeatmap;
            }
            KeyCode::Char('[') => {
                self.change_year(-1);
            }
            KeyCode::Char(']') => {
                self.change_year(1);
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
            KeyCode::Tab => {
                form.cycle_field_forward();
                return Screen::AddHabit(form);
            }
            KeyCode::Enter => {
                let name = form.name.trim().to_string();
                if name.is_empty() {
                    form.error = Some("Name cannot be empty.".to_string());
                    return Screen::AddHabit(form);
                }
                let freq = match form.parse_frequency() {
                    Ok(f) => f,
                    Err(msg) => {
                        form.error = Some(msg);
                        return Screen::AddHabit(form);
                    }
                };
                let kind = form.parse_kind();
                self.store.add_habit_kind(name, freq, self.today, kind);
                self.selected = self.store.habits.len().saturating_sub(1);
                self.status = Some("Added habit.".to_string());
                return Screen::List;
            }
            _ => match form.field {
                FormField::Name => match key.code {
                    KeyCode::Char(c) => form.name.push(c),
                    KeyCode::Backspace => {
                        form.name.pop();
                    }
                    _ => {}
                },
                FormField::Kind => match key.code {
                    KeyCode::Char(' ')
                    | KeyCode::Right
                    | KeyCode::Left
                    | KeyCode::Char('h')
                    | KeyCode::Char('l') => form.cycle_kind(),
                    _ => {}
                },
                FormField::Frequency => match key.code {
                    KeyCode::Char(' ')
                    | KeyCode::Right
                    | KeyCode::Char('l')
                    | KeyCode::Down
                    | KeyCode::Char('j') => form.cycle_freq_forward(),
                    KeyCode::Left | KeyCode::Char('h') | KeyCode::Up | KeyCode::Char('k') => {
                        form.cycle_freq_backward();
                    }
                    _ => {}
                },
                FormField::NumericValue => match key.code {
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        if form.numeric_buf.len() < 3 {
                            form.numeric_buf.push(c);
                        }
                    }
                    KeyCode::Backspace => {
                        form.numeric_buf.pop();
                    }
                    _ => {}
                },
            },
        }
        Screen::AddHabit(form)
    }

    fn handle_edit_key(&mut self, key: KeyEvent, mut form: EditForm) -> Screen {
        match key.code {
            KeyCode::Esc => {
                return Screen::List;
            }
            KeyCode::Tab => {
                form.cycle_field_forward();
                return Screen::EditHabit(form);
            }
            KeyCode::Enter => {
                let name = form.name.trim().to_string();
                if name.is_empty() {
                    form.error = Some("Name cannot be empty.".to_string());
                    return Screen::EditHabit(form);
                }
                let freq = match form.parse_frequency() {
                    Ok(f) => f,
                    Err(msg) => {
                        form.error = Some(msg);
                        return Screen::EditHabit(form);
                    }
                };
                match self.store.edit_habit(form.habit_id, Some(name), Some(freq)) {
                    Ok(()) => {
                        self.status = Some("Saved changes.".to_string());
                        return Screen::List;
                    }
                    Err(e) => {
                        form.error = Some(e.to_string());
                        return Screen::EditHabit(form);
                    }
                }
            }
            _ => match form.field {
                FormField::Name => match key.code {
                    KeyCode::Char(c) => form.name.push(c),
                    KeyCode::Backspace => {
                        form.name.pop();
                    }
                    _ => {}
                },
                FormField::Kind => {} // kind is read-only in edit
                FormField::Frequency => match key.code {
                    KeyCode::Char(' ')
                    | KeyCode::Right
                    | KeyCode::Char('l')
                    | KeyCode::Down
                    | KeyCode::Char('j') => form.cycle_freq_forward(),
                    KeyCode::Left | KeyCode::Char('h') | KeyCode::Up | KeyCode::Char('k') => {
                        form.cycle_freq_backward();
                    }
                    _ => {}
                },
                FormField::NumericValue => match key.code {
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        if form.numeric_buf.len() < 3 {
                            form.numeric_buf.push(c);
                        }
                    }
                    KeyCode::Backspace => {
                        form.numeric_buf.pop();
                    }
                    _ => {}
                },
            },
        }
        Screen::EditHabit(form)
    }

    fn handle_detail_key(&mut self, key: KeyEvent, mut state: DetailState) -> Screen {
        let habit_id = state.habit_id;
        if state.edit_mode {
            match key.code {
                KeyCode::Esc | KeyCode::Char('e') | KeyCode::Char('E') => {
                    state.edit_mode = false;
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    state.cursor = state.cursor - Duration::days(1);
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    let next = state.cursor + Duration::days(1);
                    if next <= self.today {
                        state.cursor = next;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    state.cursor = state.cursor - Duration::days(7);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let next = state.cursor + Duration::days(7);
                    if next <= self.today {
                        state.cursor = next;
                    }
                }
                KeyCode::Char(' ') | KeyCode::Enter => {
                    self.toggle_on_date(habit_id, state.cursor);
                }
                _ => {}
            }
            return Screen::Detail(state);
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => Screen::List,
            KeyCode::Char('e') | KeyCode::Char('E') => {
                state.edit_mode = true;
                state.cursor = self.today;
                self.year = self.today.year();
                self.status = Some(
                    "Edit mode: ←↑↓→ to move · space toggles · e or esc to exit.".to_string(),
                );
                Screen::Detail(state)
            }
            KeyCode::Char('[') => {
                self.change_year(-1);
                Screen::Detail(state)
            }
            KeyCode::Char(']') => {
                self.change_year(1);
                Screen::Detail(state)
            }
            _ => Screen::Detail(state),
        }
    }

    fn toggle_on_date(&mut self, habit_id: u64, date: NaiveDate) {
        let Some(habit) = self.store.habit(habit_id) else {
            return;
        };
        if date < habit.created_at {
            self.status = Some("Cannot edit dates before the habit was created.".to_string());
            return;
        }
        let kind_is_quit = matches!(habit.kind, HabitKind::Quit { .. });
        if kind_is_quit {
            let already = self.store.is_complete_on(habit_id, date);
            if already {
                let _ = self.store.clear_failure(habit_id, date);
                self.status = Some(format!("Cleared failure on {}.", date));
            } else {
                let _ = self.store.log_failure(habit_id, date);
                self.status = Some(format!("Logged failure on {}.", date));
            }
        } else {
            match self.store.toggle_completion(habit_id, date) {
                Some(true) => self.status = Some(format!("Marked {} complete.", date)),
                Some(false) => self.status = Some(format!("Cleared completion on {}.", date)),
                None => {}
            }
        }
    }

    fn handle_global_heatmap_key(&mut self, key: KeyEvent) -> Screen {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('g') | KeyCode::Enter => {
                Screen::List
            }
            KeyCode::Char('[') => {
                self.change_year(-1);
                Screen::GlobalHeatmap
            }
            KeyCode::Char(']') => {
                self.change_year(1);
                Screen::GlobalHeatmap
            }
            _ => Screen::GlobalHeatmap,
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
            restore_terminal();
        }
    }
}

fn restore_terminal() {
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
}

fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        prev(info);
    }));
}

pub fn run_app(store: &mut HabitStore) -> io::Result<()> {
    install_panic_hook();
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal: Terminal<CrosstermBackend<Stdout>> = Terminal::new(backend)?;

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
