use chrono::NaiveDate;
use habit_tracker::{storage, Frequency, HabitStore};
use tempfile::tempdir;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

#[test]
fn add_habit_assigns_unique_ids_and_persists() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);

    let id_a = store.add_habit("Read".into(), Frequency::Daily, today);
    let id_b = store.add_habit("Run".into(), Frequency::Weekly, today);
    let id_c = store.add_habit("Stretch".into(), Frequency::EveryNDays(3), today);

    assert_ne!(id_a, id_b);
    assert_ne!(id_b, id_c);
    assert_ne!(id_a, id_c);

    assert_eq!(store.habits.len(), 3);
    assert_eq!(store.habit(id_a).unwrap().name, "Read");
    assert_eq!(store.habit(id_b).unwrap().frequency, Frequency::Weekly);
    assert_eq!(
        store.habit(id_c).unwrap().frequency,
        Frequency::EveryNDays(3)
    );
}

#[test]
fn remove_habit_returns_true_on_hit_false_on_miss() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit("Read".into(), Frequency::Daily, today);

    assert!(store.remove_habit(id));
    assert!(!store.remove_habit(id));
    assert!(!store.remove_habit(9999));
    assert_eq!(store.habits.len(), 0);
}

#[test]
fn remove_habit_only_removes_the_targeted_habit() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id_a = store.add_habit("Read".into(), Frequency::Daily, today);
    let id_b = store.add_habit("Run".into(), Frequency::Weekly, today);
    let id_c = store.add_habit("Stretch".into(), Frequency::EveryNDays(3), today);

    assert!(store.remove_habit(id_b));
    assert_eq!(store.habits.len(), 2);
    assert!(store.habit(id_a).is_some(), "untouched habit must remain");
    assert!(store.habit(id_c).is_some(), "untouched habit must remain");
    assert!(store.habit(id_b).is_none(), "targeted habit must be gone");
}

#[test]
fn toggle_completion_is_idempotent() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit("Read".into(), Frequency::Daily, today);

    assert_eq!(store.toggle_completion(id, today), Some(true));
    assert!(store.habit(id).unwrap().completions.contains(&today));

    assert_eq!(store.toggle_completion(id, today), Some(false));
    assert!(!store.habit(id).unwrap().completions.contains(&today));

    assert_eq!(store.toggle_completion(9999, today), None);
}

#[test]
fn completions_on_distinct_dates_accumulate() {
    let mut store = HabitStore::new();
    let id = store.add_habit("Read".into(), Frequency::Daily, d(2026, 4, 1));
    let dates = [d(2026, 4, 1), d(2026, 4, 2), d(2026, 4, 5), d(2026, 4, 10)];
    for date in dates {
        assert_eq!(store.toggle_completion(id, date), Some(true));
    }
    let h = store.habit(id).unwrap();
    assert_eq!(h.completions.len(), dates.len());
    for date in dates {
        assert!(h.completions.contains(&date), "missing {date}");
    }
}

#[test]
fn is_due_daily() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit("Read".into(), Frequency::Daily, today);

    let h = store.habit(id).unwrap();
    assert!(h.is_due(today));

    store.toggle_completion(id, today);
    let h = store.habit(id).unwrap();
    assert!(!h.is_due(today));
    // Daily reset on the next day.
    assert!(h.is_due(today.succ_opt().unwrap()));
}

#[test]
fn is_due_weekly() {
    let mut store = HabitStore::new();
    let monday = d(2026, 4, 20);
    let id = store.add_habit("Run".into(), Frequency::Weekly, monday);

    store.toggle_completion(id, monday);
    let h = store.habit(id).unwrap();

    // Same day satisfies the window.
    assert!(!h.is_due(monday));
    // 6 days later still inside the 7-day window.
    assert!(!h.is_due(d(2026, 4, 26)));
    // Day 7 is the boundary day where the window has rolled past
    // the completion (window covers 4/21..=4/27).
    assert!(h.is_due(d(2026, 4, 27)));
}

#[test]
fn is_due_every_n_days_boundary() {
    let mut store = HabitStore::new();
    let start = d(2026, 4, 20);
    let id = store.add_habit("Stretch".into(), Frequency::EveryNDays(3), start);

    store.toggle_completion(id, start);
    let h = store.habit(id).unwrap();

    // Day-of completion: not due.
    assert!(!h.is_due(start));
    // Day +1, +2 inside 3-day window: not due.
    assert!(!h.is_due(d(2026, 4, 21)));
    assert!(!h.is_due(d(2026, 4, 22)));
    // Day +3 is the boundary: window is 4/21..=4/23, completion at 4/20 is
    // out of range, so it's due again.
    assert!(h.is_due(d(2026, 4, 23)));
}

#[test]
fn current_streak_zero_with_no_completions() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit("Read".into(), Frequency::Daily, today);
    assert_eq!(store.habit(id).unwrap().current_streak(today), 0);
}

#[test]
fn current_streak_consecutive_days() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit("Read".into(), Frequency::Daily, d(2026, 4, 20));
    for day in 21..=25 {
        store.toggle_completion(id, d(2026, 4, day));
    }
    assert_eq!(store.habit(id).unwrap().current_streak(today), 5);
}

#[test]
fn current_streak_breaks_on_gap() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit("Read".into(), Frequency::Daily, d(2026, 4, 20));
    // Completed 21, 22, (gap 23), 24, 25 => streak ending today is 24+25 = 2.
    for day in [21u32, 22, 24, 25] {
        store.toggle_completion(id, d(2026, 4, day));
    }
    assert_eq!(store.habit(id).unwrap().current_streak(today), 2);
}

#[test]
fn current_streak_zero_when_today_missed() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit("Read".into(), Frequency::Daily, d(2026, 4, 20));
    // Completed yesterday and the day before, but not today.
    store.toggle_completion(id, d(2026, 4, 23));
    store.toggle_completion(id, d(2026, 4, 24));
    assert_eq!(store.habit(id).unwrap().current_streak(today), 0);
}

#[test]
fn longest_streak_tracks_best_run() {
    let mut store = HabitStore::new();
    let id = store.add_habit("Read".into(), Frequency::Daily, d(2026, 4, 1));
    // Two runs: 4/1..4/3 (len 3) then gap, then 4/5..4/8 (len 4).
    for day in [1u32, 2, 3, 5, 6, 7, 8] {
        store.toggle_completion(id, d(2026, 4, day));
    }
    assert_eq!(store.habit(id).unwrap().longest_streak(), 4);
}

#[test]
fn save_and_load_round_trip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nested").join("habits.json");

    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit("Read".into(), Frequency::Daily, today);
    store.add_habit("Run".into(), Frequency::Weekly, today);
    store.toggle_completion(id, today);

    storage::save_to(&store, &path).expect("save should succeed");
    assert!(path.exists(), "save should create the data file");

    let loaded = storage::load_from(&path).expect("load should succeed");
    assert_eq!(loaded, store);
}

#[test]
fn load_from_missing_path_returns_empty_store() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("does-not-exist.json");
    let loaded = storage::load_from(&path).expect("missing file is not an error");
    assert_eq!(loaded, HabitStore::new());
}
