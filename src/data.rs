use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

pub const STORE_VERSION: u32 = 2;

/// How often a habit is meant to be performed.
///
/// Semantics:
/// - `Daily`: one completion required each calendar day.
/// - `Weekly`: one completion required within any rolling 7-day window.
/// - `EveryNDays(n)`: one completion required within any rolling n-day window.
/// - `NTimesPerWeek(n)`: at least n completions required within an ISO week
///   (Mon..Sun, the same week as the reference date). The current week is
///   considered "due" until n completions are logged within Mon..Sun of that
///   week. Streaks count complete past weeks plus the current week if it has
///   already met the quota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frequency {
    Daily,
    Weekly,
    EveryNDays(u32),
    NTimesPerWeek(u32),
}

/// Whether a habit is something you are trying to build (do regularly) or
/// something you are trying to quit (abstain from).
///
/// `Build` is the default and matches the v1 data model. `Quit` tracks
/// failure dates instead of completions: a streak auto-increments every day
/// from `created_at` until a failure is logged, then resets to 0 on the day
/// of a failure and starts again at 1 the next day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HabitKind {
    Build,
    Quit { failures: BTreeSet<NaiveDate> },
}

impl Default for HabitKind {
    fn default() -> Self {
        HabitKind::Build
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Habit {
    pub id: u64,
    pub name: String,
    pub frequency: Frequency,
    pub created_at: NaiveDate,
    pub completions: BTreeSet<NaiveDate>,
    #[serde(default)]
    pub kind: HabitKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HabitStore {
    pub version: u32,
    pub habits: Vec<Habit>,
    pub next_id: u64,
}

/// Errors returned by `HabitStore` mutators that can fail because the target
/// habit does not exist or the operation does not apply to that habit kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HabitError {
    NotFound(u64),
    NotApplicable(&'static str),
}

impl fmt::Display for HabitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HabitError::NotFound(id) => write!(f, "no habit with id {id}"),
            HabitError::NotApplicable(msg) => write!(f, "operation not applicable: {msg}"),
        }
    }
}

impl std::error::Error for HabitError {}

impl Default for HabitStore {
    fn default() -> Self {
        Self::new()
    }
}

impl HabitStore {
    pub fn new() -> Self {
        Self {
            version: STORE_VERSION,
            habits: Vec::new(),
            next_id: 1,
        }
    }

    pub fn add_habit(&mut self, name: String, frequency: Frequency, today: NaiveDate) -> u64 {
        self.add_habit_kind(name, frequency, today, HabitKind::Build)
    }

    pub fn add_habit_kind(
        &mut self,
        name: String,
        frequency: Frequency,
        today: NaiveDate,
        kind: HabitKind,
    ) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.habits.push(Habit {
            id,
            name,
            frequency,
            created_at: today,
            completions: BTreeSet::new(),
            kind,
        });
        id
    }

    pub fn remove_habit(&mut self, id: u64) -> bool {
        let before = self.habits.len();
        self.habits.retain(|h| h.id != id);
        self.habits.len() != before
    }

    /// Edit a habit's name and/or frequency in place. Does NOT touch
    /// `completions`, `kind` failures, `created_at`, or `id`.
    /// Returns `Err(HabitError::NotFound)` if no habit has the given id.
    pub fn edit_habit(
        &mut self,
        id: u64,
        new_name: Option<String>,
        new_frequency: Option<Frequency>,
    ) -> Result<(), HabitError> {
        let habit = self
            .habits
            .iter_mut()
            .find(|h| h.id == id)
            .ok_or(HabitError::NotFound(id))?;
        if let Some(name) = new_name {
            habit.name = name;
        }
        if let Some(freq) = new_frequency {
            habit.frequency = freq;
        }
        Ok(())
    }

    /// Toggle completion for a Build habit on `date`.
    /// Returns `Some(true)` if newly inserted, `Some(false)` if removed,
    /// or `None` if no habit with that id exists OR the habit is a Quit habit.
    pub fn toggle_completion(&mut self, id: u64, date: NaiveDate) -> Option<bool> {
        let habit = self.habits.iter_mut().find(|h| h.id == id)?;
        if !matches!(habit.kind, HabitKind::Build) {
            return None;
        }
        if habit.completions.remove(&date) {
            Some(false)
        } else {
            habit.completions.insert(date);
            Some(true)
        }
    }

    /// Log a failure for a Quit habit on `date`. Idempotent: logging the same
    /// date twice leaves the set unchanged but still returns `Ok(())`.
    /// Returns `Err(HabitError::NotFound)` if no habit has the id, or
    /// `Err(HabitError::NotApplicable)` if the habit is a Build habit.
    pub fn log_failure(&mut self, id: u64, date: NaiveDate) -> Result<(), HabitError> {
        let habit = self
            .habits
            .iter_mut()
            .find(|h| h.id == id)
            .ok_or(HabitError::NotFound(id))?;
        match &mut habit.kind {
            HabitKind::Quit { failures } => {
                failures.insert(date);
                Ok(())
            }
            HabitKind::Build => Err(HabitError::NotApplicable(
                "log_failure on a Build habit",
            )),
        }
    }

    /// Clear a previously-logged failure for a Quit habit on `date`.
    /// Returns `Ok(true)` if a failure was removed, `Ok(false)` if there was
    /// no failure on that date. Errors mirror `log_failure`.
    pub fn clear_failure(&mut self, id: u64, date: NaiveDate) -> Result<bool, HabitError> {
        let habit = self
            .habits
            .iter_mut()
            .find(|h| h.id == id)
            .ok_or(HabitError::NotFound(id))?;
        match &mut habit.kind {
            HabitKind::Quit { failures } => Ok(failures.remove(&date)),
            HabitKind::Build => Err(HabitError::NotApplicable(
                "clear_failure on a Build habit",
            )),
        }
    }

    pub fn habit(&self, id: u64) -> Option<&Habit> {
        self.habits.iter().find(|h| h.id == id)
    }

    pub fn habit_mut(&mut self, id: u64) -> Option<&mut Habit> {
        self.habits.iter_mut().find(|h| h.id == id)
    }

    /// True iff the habit with `id` has a completion exactly on `date`.
    /// For Quit habits, returns whether that date is logged as a failure.
    /// Returns false if no habit with that id exists.
    pub fn is_complete_on(&self, id: u64, date: NaiveDate) -> bool {
        let Some(habit) = self.habit(id) else {
            return false;
        };
        match &habit.kind {
            HabitKind::Build => habit.completions.contains(&date),
            HabitKind::Quit { failures } => failures.contains(&date),
        }
    }

    /// Current streak for the habit with `id`, anchored at `today`.
    /// Returns 0 if no habit with that id exists.
    pub fn current_streak(&self, id: u64, today: NaiveDate) -> u32 {
        self.habit(id).map(|h| h.current_streak(today)).unwrap_or(0)
    }

    /// Completion dates (or failure dates for Quit habits) for the habit with
    /// `id` falling within `[from, to]` (inclusive on both ends), in ascending
    /// order. Returns an empty vec if no habit with that id exists or
    /// `from > to`.
    pub fn completions_in_range(
        &self,
        id: u64,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Vec<NaiveDate> {
        let Some(habit) = self.habit(id) else {
            return Vec::new();
        };
        if from > to {
            return Vec::new();
        }
        let set: &BTreeSet<NaiveDate> = match &habit.kind {
            HabitKind::Build => &habit.completions,
            HabitKind::Quit { failures } => failures,
        };
        set.range(from..=to).copied().collect()
    }
}

/// Monday of the ISO week containing `date`.
fn iso_week_start(date: NaiveDate) -> NaiveDate {
    let weekday = date.weekday();
    let offset = weekday.num_days_from_monday() as i64;
    date - Duration::days(offset)
}

impl Habit {
    /// Period length in days for rolling-window streak / due calculations.
    /// Not meaningful for `NTimesPerWeek` (which uses ISO weeks).
    fn period_days(&self) -> u32 {
        match self.frequency {
            Frequency::Daily => 1,
            Frequency::Weekly => 7,
            Frequency::EveryNDays(n) => n.max(1),
            Frequency::NTimesPerWeek(_) => 7,
        }
    }

    /// True if the habit still needs action for the period anchored at `date`.
    ///
    /// - Build / Daily: no completion exactly on `date`.
    /// - Build / Weekly: no completion in the rolling 7-day window ending on `date`.
    /// - Build / EveryNDays(n): no completion in the rolling n-day window ending on `date`.
    /// - Build / NTimesPerWeek(n): fewer than n completions in the ISO week
    ///   (Mon..Sun) containing `date`.
    /// - Quit (any frequency): always false. Quit habits are passive — there
    ///   is nothing to "do today"; the only action is logging a failure.
    pub fn is_due(&self, date: NaiveDate) -> bool {
        if matches!(self.kind, HabitKind::Quit { .. }) {
            return false;
        }
        match self.frequency {
            Frequency::Daily => !self.completions.contains(&date),
            Frequency::Weekly | Frequency::EveryNDays(_) => {
                let period = self.period_days() as i64;
                let window_start = date - Duration::days(period - 1);
                self.completions.range(window_start..=date).next().is_none()
            }
            Frequency::NTimesPerWeek(n) => {
                let n = n as usize;
                if n == 0 {
                    return false;
                }
                let monday = iso_week_start(date);
                let sunday = monday + Duration::days(6);
                self.completions.range(monday..=sunday).count() < n
            }
        }
    }

    /// Current streak ending at `today`.
    ///
    /// - Quit: number of consecutive abstinence days ending today. Equals
    ///   `(today - anchor).num_days() + 1`, where `anchor` is the day after
    ///   the most recent failure on or before `today` (or `created_at` if
    ///   there are no such failures). On the day of a failure the streak is
    ///   0; the next day it is 1. Before `created_at` it is 0.
    /// - Build / Daily: consecutive completed days ending today.
    /// - Build / Weekly | EveryNDays(n): consecutive rolling n-day windows
    ///   ending today, today-n, today-2n, ... that each contain ≥1 completion.
    /// - Build / NTimesPerWeek(n): consecutive past ISO weeks (each with ≥n
    ///   completions) ending at the current ISO week, plus 1 if the current
    ///   ISO week already has ≥n completions. The current week does not break
    ///   the streak just because it is mid-week — it simply doesn't count
    ///   toward the streak until n is reached.
    pub fn current_streak(&self, today: NaiveDate) -> u32 {
        if let HabitKind::Quit { failures } = &self.kind {
            if today < self.created_at {
                return 0;
            }
            let last_failure = failures.range(..=today).next_back().copied();
            let anchor = match last_failure {
                Some(f) if f >= today => return 0,
                Some(f) => f + Duration::days(1),
                None => self.created_at,
            };
            if today < anchor {
                return 0;
            }
            let days = (today - anchor).num_days() + 1;
            return u32::try_from(days.max(0)).unwrap_or(u32::MAX);
        }

        if let Frequency::NTimesPerWeek(n) = self.frequency {
            return self.weekly_quota_streak(today, n);
        }

        if self.completions.is_empty() {
            return 0;
        }
        let period = self.period_days() as i64;
        let mut streak = 0u32;
        let mut window_end = today;
        loop {
            let window_start = window_end - Duration::days(period - 1);
            let hit = self
                .completions
                .range(window_start..=window_end)
                .next()
                .is_some();
            if !hit {
                break;
            }
            streak = streak.saturating_add(1);
            window_end = window_start - Duration::days(1);
        }
        streak
    }

    fn weekly_quota_streak(&self, today: NaiveDate, n: u32) -> u32 {
        if n == 0 || self.completions.is_empty() {
            return 0;
        }
        let n = n as usize;
        let mut streak = 0u32;
        let mut monday = iso_week_start(today);

        let sunday = monday + Duration::days(6);
        let count = self.completions.range(monday..=sunday).count();
        if count >= n {
            streak = streak.saturating_add(1);
        }

        monday -= Duration::days(7);
        loop {
            let sunday = monday + Duration::days(6);
            let count = self.completions.range(monday..=sunday).count();
            if count < n {
                break;
            }
            streak = streak.saturating_add(1);
            monday -= Duration::days(7);
        }
        streak
    }

    /// Longest run of consecutive periods (anywhere in history) that each
    /// contain at least one completion (or, for Quit, the longest abstinence
    /// run between failures).
    ///
    /// - Build / Daily: longest run of consecutive completed days.
    /// - Build / Weekly | EveryNDays(n): periods are anchored to the earliest
    ///   completion (period_0 = [first, first + n - 1]); a hit period has
    ///   ≥1 completion. Result is the longest run of consecutive hit periods.
    /// - Build / NTimesPerWeek(n): longest run of consecutive ISO weeks
    ///   (between the first and last completion) that each have ≥n completions.
    /// - Quit: longest abstinence span. Spans are
    ///   `[created_at .. first_failure - 1]`,
    ///   `[failure_i + 1 .. failure_{i+1} - 1]`, and
    ///   `[last_failure + 1 .. today_or_anywhere]`. Without a `today`
    ///   reference, the trailing open span is bounded by the latest known
    ///   date (last failure or created_at). For an ongoing streak that
    ///   exceeds the historical best, callers should use `current_streak`.
    pub fn longest_streak(&self) -> u32 {
        if let HabitKind::Quit { failures } = &self.kind {
            return self.quit_longest_streak(failures);
        }

        if let Frequency::NTimesPerWeek(n) = self.frequency {
            return self.weekly_quota_longest(n);
        }

        if self.completions.is_empty() {
            return 0;
        }
        let period = self.period_days() as i64;
        let first = *self.completions.iter().next().unwrap();
        let last = *self.completions.iter().next_back().unwrap();

        let mut best = 0u32;
        let mut current = 0u32;
        let mut window_start = first;
        while window_start <= last {
            let window_end = window_start + Duration::days(period - 1);
            let hit = self
                .completions
                .range(window_start..=window_end)
                .next()
                .is_some();
            if hit {
                current = current.saturating_add(1);
                if current > best {
                    best = current;
                }
            } else {
                current = 0;
            }
            window_start = window_end + Duration::days(1);
        }
        best
    }

    fn weekly_quota_longest(&self, n: u32) -> u32 {
        if n == 0 || self.completions.is_empty() {
            return 0;
        }
        let n = n as usize;
        let first = *self.completions.iter().next().unwrap();
        let last = *self.completions.iter().next_back().unwrap();
        let mut monday = iso_week_start(first);
        let last_monday = iso_week_start(last);

        let mut best = 0u32;
        let mut current = 0u32;
        while monday <= last_monday {
            let sunday = monday + Duration::days(6);
            let count = self.completions.range(monday..=sunday).count();
            if count >= n {
                current = current.saturating_add(1);
                if current > best {
                    best = current;
                }
            } else {
                current = 0;
            }
            monday += Duration::days(7);
        }
        best
    }

    fn quit_longest_streak(&self, failures: &BTreeSet<NaiveDate>) -> u32 {
        // Span lengths in days between consecutive boundaries.
        // Boundaries: created_at, each failure, and the latest known date.
        let mut best: i64 = 0;
        let mut prev_anchor = self.created_at;
        let mut latest = self.created_at;

        for &fail in failures.iter() {
            if fail < self.created_at {
                continue;
            }
            // Span runs from prev_anchor up to (but not including) the failure.
            let span = (fail - prev_anchor).num_days();
            if span > best {
                best = span;
            }
            prev_anchor = fail + Duration::days(1);
            if fail > latest {
                latest = fail;
            }
        }
        // Trailing open span up to `latest` (the most recent known date).
        let trailing = (latest - prev_anchor).num_days() + 1;
        if trailing > best {
            best = trailing;
        }
        u32::try_from(best.max(0)).unwrap_or(u32::MAX)
    }
}
