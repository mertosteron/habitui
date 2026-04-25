use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const STORE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frequency {
    Daily,
    Weekly,
    EveryNDays(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Habit {
    pub id: u64,
    pub name: String,
    pub frequency: Frequency,
    pub created_at: NaiveDate,
    pub completions: BTreeSet<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HabitStore {
    pub version: u32,
    pub habits: Vec<Habit>,
    pub next_id: u64,
}

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
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.habits.push(Habit {
            id,
            name,
            frequency,
            created_at: today,
            completions: BTreeSet::new(),
        });
        id
    }

    pub fn remove_habit(&mut self, id: u64) -> bool {
        let before = self.habits.len();
        self.habits.retain(|h| h.id != id);
        self.habits.len() != before
    }

    /// Toggle completion for a habit on `date`.
    /// Returns `Some(true)` if newly inserted, `Some(false)` if removed,
    /// or `None` if no habit with that id exists.
    pub fn toggle_completion(&mut self, id: u64, date: NaiveDate) -> Option<bool> {
        let habit = self.habits.iter_mut().find(|h| h.id == id)?;
        if habit.completions.remove(&date) {
            Some(false)
        } else {
            habit.completions.insert(date);
            Some(true)
        }
    }

    pub fn habit(&self, id: u64) -> Option<&Habit> {
        self.habits.iter().find(|h| h.id == id)
    }

    pub fn habit_mut(&mut self, id: u64) -> Option<&mut Habit> {
        self.habits.iter_mut().find(|h| h.id == id)
    }
}

impl Habit {
    /// Period length in days for streak / due-window calculations.
    fn period_days(&self) -> u32 {
        match self.frequency {
            Frequency::Daily => 1,
            Frequency::Weekly => 7,
            Frequency::EveryNDays(n) => n.max(1),
        }
    }

    /// True if the habit still needs to be completed for the period ending on `date`.
    ///
    /// - Daily: true iff there is no completion exactly on `date`.
    /// - Weekly: true iff there is no completion in the 7-day window ending on `date` (inclusive).
    /// - EveryNDays(n): true iff there is no completion in the n-day window ending on `date`.
    pub fn is_due(&self, date: NaiveDate) -> bool {
        let period = self.period_days();
        if period <= 1 {
            return !self.completions.contains(&date);
        }
        let window_start = date - Duration::days((period as i64) - 1);
        !self
            .completions
            .range(window_start..=date)
            .next()
            .is_some()
    }

    /// Consecutive periods (ending at `today`) that have at least one completion.
    ///
    /// - Daily: consecutive days ending today.
    /// - Weekly / EveryNDays(n): consecutive n-day windows ending today, today-n, today-2n, ...
    pub fn current_streak(&self, today: NaiveDate) -> u32 {
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

    /// Longest run of consecutive periods (anywhere in history) that each contain
    /// at least one completion.
    ///
    /// For Daily this is the longest run of consecutive completed days.
    /// For Weekly / EveryNDays(n), periods are anchored to the earliest completion
    /// (period_0 = [first_completion, first_completion + n - 1]) and a "hit" period
    /// is one with ≥1 completion. The result is the longest run of consecutive hit
    /// periods.
    pub fn longest_streak(&self) -> u32 {
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
}
