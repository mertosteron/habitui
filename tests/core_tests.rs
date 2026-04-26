use chrono::NaiveDate;
use habitui::{storage, Frequency, HabitKind, HabitStore};
use std::collections::BTreeSet;
use std::fs;
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

// -------------------------------------------------------------------------
// v2 additions: edit_habit, HabitKind::Quit, Frequency::NTimesPerWeek,
// store migration v1 -> v2.
// -------------------------------------------------------------------------

#[test]
fn edit_habit_preserves_completions_streak_id_and_created_at() {
    let mut store = HabitStore::new();
    let created = d(2026, 4, 20);
    let today = d(2026, 4, 25);
    let id = store.add_habit("Read".into(), Frequency::Daily, created);
    for day in 21..=25 {
        store.toggle_completion(id, d(2026, 4, day));
    }
    let streak_before = store.habit(id).unwrap().current_streak(today);
    assert_eq!(streak_before, 5, "precondition: 5-day streak");
    let completions_before = store.habit(id).unwrap().completions.clone();

    store
        .edit_habit(id, Some("Read books".into()), Some(Frequency::Weekly))
        .expect("edit must succeed");

    let h = store.habit(id).unwrap();
    assert_eq!(h.id, id, "id must not change");
    assert_eq!(h.name, "Read books");
    assert_eq!(h.frequency, Frequency::Weekly);
    assert_eq!(h.created_at, created, "created_at must not change");
    assert_eq!(
        h.completions, completions_before,
        "completions must be preserved verbatim"
    );
    assert!(matches!(h.kind, HabitKind::Build));
}

#[test]
fn edit_habit_with_no_changes_is_ok() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit("Read".into(), Frequency::Daily, today);
    let snapshot = store.habit(id).unwrap().clone();

    store
        .edit_habit(id, None, None)
        .expect("edit with no changes is still Ok");

    assert_eq!(store.habit(id).unwrap(), &snapshot);
}

#[test]
fn edit_habit_returns_not_found_for_missing_id() {
    let mut store = HabitStore::new();
    let result = store.edit_habit(9999, Some("X".into()), None);
    assert!(matches!(
        result,
        Err(habitui::data::HabitError::NotFound(9999))
    ));
}

#[test]
fn edit_habit_does_not_touch_quit_failures() {
    let mut store = HabitStore::new();
    let created = d(2026, 4, 20);
    let id = store.add_habit_kind(
        "No sugar".into(),
        Frequency::Daily,
        created,
        HabitKind::Quit {
            failures: BTreeSet::new(),
        },
    );
    store.log_failure(id, d(2026, 4, 22)).unwrap();
    store.log_failure(id, d(2026, 4, 24)).unwrap();

    store
        .edit_habit(id, Some("No added sugar".into()), Some(Frequency::Weekly))
        .unwrap();

    let h = store.habit(id).unwrap();
    assert_eq!(h.name, "No added sugar");
    assert_eq!(h.frequency, Frequency::Weekly);
    match &h.kind {
        HabitKind::Quit { failures } => {
            assert_eq!(failures.len(), 2);
            assert!(failures.contains(&d(2026, 4, 22)));
            assert!(failures.contains(&d(2026, 4, 24)));
        }
        _ => panic!("kind must remain Quit"),
    }
}

#[test]
fn edit_between_frequency_variants_keeps_history() {
    let mut store = HabitStore::new();
    let created = d(2026, 4, 1);
    let id = store.add_habit("Workout".into(), Frequency::Daily, created);
    let dates = [
        d(2026, 4, 5),
        d(2026, 4, 8),
        d(2026, 4, 12),
        d(2026, 4, 15),
        d(2026, 4, 20),
    ];
    for date in dates {
        store.toggle_completion(id, date);
    }
    let snapshot = store.habit(id).unwrap().completions.clone();

    // Daily -> NTimesPerWeek -> EveryNDays -> back to Daily.
    store
        .edit_habit(id, None, Some(Frequency::NTimesPerWeek(3)))
        .unwrap();
    assert_eq!(store.habit(id).unwrap().completions, snapshot);

    store
        .edit_habit(id, None, Some(Frequency::EveryNDays(3)))
        .unwrap();
    assert_eq!(store.habit(id).unwrap().completions, snapshot);

    store
        .edit_habit(id, None, Some(Frequency::Daily))
        .unwrap();
    let h = store.habit(id).unwrap();
    assert_eq!(h.completions, snapshot);
    assert_eq!(h.created_at, created);
    assert_eq!(h.id, id);
}

#[test]
fn quit_habit_auto_increments_streak_with_no_failures() {
    let mut store = HabitStore::new();
    let created = d(2026, 4, 20);
    let id = store.add_habit_kind(
        "No sugar".into(),
        Frequency::Daily,
        created,
        HabitKind::Quit {
            failures: BTreeSet::new(),
        },
    );

    // Day-of-creation streak is 1.
    assert_eq!(store.habit(id).unwrap().current_streak(created), 1);

    // 5 days later (created + 5) the streak is 6 (created counts as day 1).
    let today = d(2026, 4, 25);
    assert_eq!(store.habit(id).unwrap().current_streak(today), 6);

    // Before created_at the streak is 0.
    assert_eq!(
        store.habit(id).unwrap().current_streak(d(2026, 4, 19)),
        0
    );
}

#[test]
fn quit_habit_failure_resets_streak() {
    let mut store = HabitStore::new();
    let created = d(2026, 4, 20);
    let id = store.add_habit_kind(
        "No sugar".into(),
        Frequency::Daily,
        created,
        HabitKind::Quit {
            failures: BTreeSet::new(),
        },
    );

    // Failure on day F = 4/22.
    store.log_failure(id, d(2026, 4, 22)).unwrap();

    // On the day of failure the streak is 0.
    assert_eq!(store.habit(id).unwrap().current_streak(d(2026, 4, 22)), 0);

    // Day after failure: streak is 1.
    assert_eq!(store.habit(id).unwrap().current_streak(d(2026, 4, 23)), 1);

    // 3 days after failure: today's streak counts only days since F.
    let today = d(2026, 4, 25);
    assert_eq!(store.habit(id).unwrap().current_streak(today), 3);
}

#[test]
fn quit_habit_toggle_completion_returns_none() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit_kind(
        "No sugar".into(),
        Frequency::Daily,
        today,
        HabitKind::Quit {
            failures: BTreeSet::new(),
        },
    );
    assert_eq!(store.toggle_completion(id, today), None);
}

#[test]
fn quit_habit_log_and_clear_failure() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit_kind(
        "No sugar".into(),
        Frequency::Daily,
        today,
        HabitKind::Quit {
            failures: BTreeSet::new(),
        },
    );

    // log_failure is idempotent.
    store.log_failure(id, today).unwrap();
    store.log_failure(id, today).unwrap();
    assert!(store.is_complete_on(id, today));

    // clear_failure returns true if removed, false otherwise.
    assert_eq!(store.clear_failure(id, today), Ok(true));
    assert_eq!(store.clear_failure(id, today), Ok(false));
    assert!(!store.is_complete_on(id, today));
}

#[test]
fn build_habit_log_failure_is_not_applicable() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit("Read".into(), Frequency::Daily, today);

    let err = store.log_failure(id, today).unwrap_err();
    assert!(matches!(err, habitui::data::HabitError::NotApplicable(_)));

    let err = store.clear_failure(id, today).unwrap_err();
    assert!(matches!(err, habitui::data::HabitError::NotApplicable(_)));
}

#[test]
fn quit_habit_is_due_always_false() {
    let mut store = HabitStore::new();
    let today = d(2026, 4, 25);
    let id = store.add_habit_kind(
        "No sugar".into(),
        Frequency::Daily,
        today,
        HabitKind::Quit {
            failures: BTreeSet::new(),
        },
    );
    let h = store.habit(id).unwrap();
    assert!(!h.is_due(today));
    assert!(!h.is_due(today.succ_opt().unwrap()));
}

#[test]
fn n_times_per_week_is_due() {
    // ISO week containing 2026-04-22 (Wednesday) is Mon 4/20 .. Sun 4/26.
    let mut store = HabitStore::new();
    let id = store.add_habit("Gym".into(), Frequency::NTimesPerWeek(3), d(2026, 4, 20));

    // 0 completions: due.
    assert!(store.habit(id).unwrap().is_due(d(2026, 4, 22)));
    store.toggle_completion(id, d(2026, 4, 20));
    store.toggle_completion(id, d(2026, 4, 22));
    // Only 2 < 3 completions: still due.
    assert!(store.habit(id).unwrap().is_due(d(2026, 4, 22)));
    store.toggle_completion(id, d(2026, 4, 24));
    // 3 completions in the ISO week: not due.
    assert!(!store.habit(id).unwrap().is_due(d(2026, 4, 24)));
    // Next ISO week starts 4/27: due again.
    assert!(store.habit(id).unwrap().is_due(d(2026, 4, 27)));
}

#[test]
fn n_times_per_week_current_streak() {
    // n=3.
    // Week A (Mon 4/6  .. Sun 4/12): 3 completions
    // Week B (Mon 4/13 .. Sun 4/19): 4 completions
    // Week C (Mon 4/20 .. Sun 4/26): 2 completions  (current week, today=4/22)
    // Expected streak: 2 (last 2 *full* weeks B and A counted; C below quota does not break).
    let mut store = HabitStore::new();
    let id = store.add_habit("Gym".into(), Frequency::NTimesPerWeek(3), d(2026, 4, 1));
    for date in [d(2026, 4, 6), d(2026, 4, 8), d(2026, 4, 10)] {
        store.toggle_completion(id, date);
    }
    for date in [
        d(2026, 4, 13),
        d(2026, 4, 14),
        d(2026, 4, 16),
        d(2026, 4, 18),
    ] {
        store.toggle_completion(id, date);
    }
    for date in [d(2026, 4, 20), d(2026, 4, 22)] {
        store.toggle_completion(id, date);
    }
    let today = d(2026, 4, 22);
    assert_eq!(store.habit(id).unwrap().current_streak(today), 2);

    // Once the current week reaches the quota, streak is 3.
    store.toggle_completion(id, d(2026, 4, 23));
    assert_eq!(store.habit(id).unwrap().current_streak(today.succ_opt().unwrap()), 3);
}

#[test]
fn n_times_per_week_longest_streak() {
    // n=3. Three consecutive weeks at quota, then a gap, then two weeks.
    let mut store = HabitStore::new();
    let id = store.add_habit("Gym".into(), Frequency::NTimesPerWeek(3), d(2026, 1, 1));

    let weeks = [
        // Run of 3 weeks at quota.
        [d(2026, 1, 5), d(2026, 1, 7), d(2026, 1, 9)],
        [d(2026, 1, 12), d(2026, 1, 14), d(2026, 1, 16)],
        [d(2026, 1, 19), d(2026, 1, 21), d(2026, 1, 23)],
        // Gap week (only 1 completion).
        [d(2026, 1, 26), d(2026, 1, 26), d(2026, 1, 26)],
        // Run of 2 weeks at quota.
        [d(2026, 2, 2), d(2026, 2, 4), d(2026, 2, 6)],
        [d(2026, 2, 9), d(2026, 2, 11), d(2026, 2, 13)],
    ];
    for (i, w) in weeks.iter().enumerate() {
        if i == 3 {
            store.toggle_completion(id, w[0]);
        } else {
            for date in w {
                store.toggle_completion(id, *date);
            }
        }
    }
    assert_eq!(store.habit(id).unwrap().longest_streak(), 3);
}

#[test]
fn store_round_trip_with_quit_and_all_frequencies() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("habits.json");
    let today = d(2026, 4, 25);

    let mut store = HabitStore::new();
    let read = store.add_habit("Read".into(), Frequency::Daily, today);
    let run = store.add_habit("Run".into(), Frequency::Weekly, today);
    let stretch = store.add_habit("Stretch".into(), Frequency::EveryNDays(3), today);
    let gym = store.add_habit("Gym".into(), Frequency::NTimesPerWeek(3), today);
    let quit = store.add_habit_kind(
        "No sugar".into(),
        Frequency::Daily,
        today,
        HabitKind::Quit {
            failures: BTreeSet::new(),
        },
    );
    store.toggle_completion(read, today);
    store.toggle_completion(run, today);
    store.toggle_completion(stretch, today);
    store.toggle_completion(gym, today);
    store.log_failure(quit, today).unwrap();

    storage::save_to(&store, &path).expect("save should succeed");
    let loaded = storage::load_from(&path).expect("load should succeed");
    assert_eq!(loaded, store);
}

#[test]
fn v1_store_json_loads_as_v2() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("habits.json");

    // Hand-crafted v1 JSON: no `kind` field, version 1.
    let v1 = r#"{
        "version": 1,
        "habits": [
            {
                "id": 1,
                "name": "Read",
                "frequency": "Daily",
                "created_at": "2026-04-20",
                "completions": ["2026-04-20", "2026-04-21", "2026-04-22"]
            },
            {
                "id": 2,
                "name": "Run",
                "frequency": {"EveryNDays": 3},
                "created_at": "2026-04-15",
                "completions": []
            }
        ],
        "next_id": 3
    }"#;
    fs::write(&path, v1).unwrap();

    let loaded = storage::load_from(&path).expect("v1 must load without error");

    // Version is bumped in memory.
    assert_eq!(loaded.version, habitui::STORE_VERSION);
    assert_eq!(loaded.version, 2);

    // Both habits get HabitKind::Build via serde default.
    assert_eq!(loaded.habits.len(), 2);
    for h in &loaded.habits {
        assert!(matches!(h.kind, HabitKind::Build));
    }

    let read = loaded.habit(1).unwrap();
    assert_eq!(read.name, "Read");
    assert_eq!(read.frequency, Frequency::Daily);
    assert_eq!(read.completions.len(), 3);

    let run = loaded.habit(2).unwrap();
    assert_eq!(run.frequency, Frequency::EveryNDays(3));
    assert!(run.completions.is_empty());
}
