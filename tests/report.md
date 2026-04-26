# habitui — QA & Packaging Report

Date: 2026-04-26
Owner: qa-packager
Verdict: **PASS**

## Test summary

31 integration tests in `tests/core_tests.rs`, all passing under
`cargo test --release --locked`. 16 are pre-existing v1 tests retained
verbatim; 15 are new and cover the v2 contract (edit, Quit habits,
NTimesPerWeek, v1 → v2 migration).

Cargo test summary (release):

```
running 31 tests
test add_habit_assigns_unique_ids_and_persists ... ok
test build_habit_log_failure_is_not_applicable ... ok
test completions_on_distinct_dates_accumulate ... ok
test current_streak_breaks_on_gap ... ok
test current_streak_consecutive_days ... ok
test current_streak_zero_when_today_missed ... ok
test current_streak_zero_with_no_completions ... ok
test edit_between_frequency_variants_keeps_history ... ok
test edit_habit_does_not_touch_quit_failures ... ok
test edit_habit_preserves_completions_streak_id_and_created_at ... ok
test edit_habit_returns_not_found_for_missing_id ... ok
test edit_habit_with_no_changes_is_ok ... ok
test is_due_daily ... ok
test is_due_every_n_days_boundary ... ok
test is_due_weekly ... ok
test load_from_missing_path_returns_empty_store ... ok
test longest_streak_tracks_best_run ... ok
test n_times_per_week_current_streak ... ok
test n_times_per_week_is_due ... ok
test n_times_per_week_longest_streak ... ok
test quit_habit_auto_increments_streak_with_no_failures ... ok
test quit_habit_failure_resets_streak ... ok
test quit_habit_is_due_always_false ... ok
test quit_habit_log_and_clear_failure ... ok
test quit_habit_toggle_completion_returns_none ... ok
test remove_habit_only_removes_the_targeted_habit ... ok
test remove_habit_returns_true_on_hit_false_on_miss ... ok
test save_and_load_round_trip ... ok
test store_round_trip_with_quit_and_all_frequencies ... ok
test toggle_completion_is_idempotent ... ok
test v1_store_json_loads_as_v2 ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Lib unit tests, bin unit tests, and doc tests all run with 0 tests defined
and pass. `cargo build --release --locked` produces `target/release/habitui`.

## Coverage map

| # | Test | Result | Note |
|---|------|--------|------|
| 1  | `add_habit_assigns_unique_ids_and_persists` | pass | v1 retained: ids unique, store mutates. |
| 2  | `remove_habit_returns_true_on_hit_false_on_miss` | pass | v1 retained. |
| 3  | `remove_habit_only_removes_the_targeted_habit` | pass | v1 retained. |
| 4  | `toggle_completion_is_idempotent` | pass | v1 retained: toggle twice = no-op. |
| 5  | `completions_on_distinct_dates_accumulate` | pass | v1 retained. |
| 6  | `is_due_daily` | pass | v1 retained: Daily window. |
| 7  | `is_due_weekly` | pass | v1 retained: 7-day rolling window. |
| 8  | `is_due_every_n_days_boundary` | pass | v1 retained: n-day rolling window. |
| 9  | `current_streak_zero_with_no_completions` | pass | v1 retained. |
| 10 | `current_streak_consecutive_days` | pass | v1 retained. |
| 11 | `current_streak_breaks_on_gap` | pass | v1 retained. |
| 12 | `current_streak_zero_when_today_missed` | pass | v1 retained. |
| 13 | `longest_streak_tracks_best_run` | pass | v1 retained. |
| 14 | `save_and_load_round_trip` | pass | v1 retained. |
| 15 | `load_from_missing_path_returns_empty_store` | pass | v1 retained. |
| 16 | `edit_habit_preserves_completions_streak_id_and_created_at` | pass | v2: edit Daily→Weekly with 5-day streak; completions, id, created_at intact. |
| 17 | `edit_habit_with_no_changes_is_ok` | pass | v2: `edit_habit(id, None, None)` is Ok and a no-op. |
| 18 | `edit_habit_returns_not_found_for_missing_id` | pass | v2: `Err(NotFound(id))` for unknown id. |
| 19 | `edit_habit_does_not_touch_quit_failures` | pass | v2: editing a Quit habit's name/freq leaves failures intact. |
| 20 | `edit_between_frequency_variants_keeps_history` | pass | v2: Daily↔NTimesPerWeek↔EveryNDays cycle preserves completions. |
| 21 | `quit_habit_auto_increments_streak_with_no_failures` | pass | v2: streak = days since created_at, inclusive (created day = 1). |
| 22 | `quit_habit_failure_resets_streak` | pass | v2: failure on day F → streak on day F is 0, day F+1 is 1; today counts only days since F. |
| 23 | `quit_habit_toggle_completion_returns_none` | pass | v2: toggle_completion is N/A on Quit habits. |
| 24 | `quit_habit_log_and_clear_failure` | pass | v2: log_failure idempotent; clear_failure returns Ok(true)/Ok(false). |
| 25 | `build_habit_log_failure_is_not_applicable` | pass | v2: log_failure / clear_failure on Build → `Err(NotApplicable)`. |
| 26 | `quit_habit_is_due_always_false` | pass | v2: Quit is_due is always false. |
| 27 | `n_times_per_week_is_due` | pass | v2: due iff fewer than n completions in the ISO week. |
| 28 | `n_times_per_week_current_streak` | pass | v2: under-quota current week doesn't break streak; full prior weeks still count. |
| 29 | `n_times_per_week_longest_streak` | pass | v2: longest run of consecutive ISO weeks at quota. |
| 30 | `store_round_trip_with_quit_and_all_frequencies` | pass | v2: round-trip with Build + Quit habits and all four Frequency variants. |
| 31 | `v1_store_json_loads_as_v2` | pass | v2: hand-crafted v1 JSON loads, version bumps to 2, kind defaults to Build. |

## PKGBUILD lint output

`makepkg --printsrcinfo` (PKGBUILD syntax check; exit code 0):

```
pkgbase = habitui
	pkgdesc = Terminal UI habit tracker written in Rust
	pkgver = 0.1.0
	pkgrel = 1
	url = https://example.invalid/habitui
	arch = x86_64
	license = MIT
	makedepends = cargo
	makedepends = rust
	depends = gcc-libs
	depends = glibc

pkgname = habitui
```

`namcap PKGBUILD` was **skipped** — `namcap` is not installed on the build
host (`which namcap` returns nothing). PKGBUILD was hand-audited against
Arch packaging guidelines:

- `pkgname=habitui` is lower-case alphanumeric. ✓
- `pkgver=0.1.0` matches `Cargo.toml`. ✓
- `pkgrel=1`, `arch=('x86_64')`. ✓
- `license=('MIT')` matches the actual `LICENSE` file at repo root (MIT License). ✓
- `makedepends=('cargo' 'rust')`. ✓
- `depends=('gcc-libs' 'glibc')` matches typical dynamic linkage of a Rust release binary on Arch. ✓
- `build()` runs `cargo build --release --locked`. ✓
- `check()` runs `cargo test --release --locked`. ✓
- `package()` installs the binary at `/usr/bin/habitui` and the LICENSE at `/usr/share/licenses/habitui/LICENSE`. ✓

## Final verdict

**PASS.** All 31 tests pass under `cargo test --release --locked`. PKGBUILD
parses cleanly via `makepkg --printsrcinfo` and was renamed from
`habit-tracker` to `habitui` to match the renamed crate and binary. Ready
for `makepkg -si` from the repo root.
