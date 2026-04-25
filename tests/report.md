# habit-tracker — QA & Packaging Report

Date: 2026-04-25
Owner: qa-arch-packager
Verdict: **PASS**

## Test summary

15 integration tests in `tests/core_tests.rs`, all passing in both debug and release.

```
running 15 tests
test add_habit_assigns_unique_ids_and_persists ... ok
test completions_on_distinct_dates_accumulate ... ok
test current_streak_breaks_on_gap ... ok
test current_streak_consecutive_days ... ok
test current_streak_zero_when_today_missed ... ok
test current_streak_zero_with_no_completions ... ok
test is_due_daily ... ok
test is_due_every_n_days_boundary ... ok
test is_due_weekly ... ok
test longest_streak_tracks_best_run ... ok
test load_from_missing_path_returns_empty_store ... ok
test remove_habit_only_removes_the_targeted_habit ... ok
test remove_habit_returns_true_on_hit_false_on_miss ... ok
test toggle_completion_is_idempotent ... ok
test save_and_load_round_trip ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Run with `cargo test` (debug) or `cargo test --release`. Both succeed cleanly.

## Coverage map (vs. brief)

| Brief requirement                                                | Test(s) |
|------------------------------------------------------------------|---------|
| `add_habit` returns unique id, store contains habit              | `add_habit_assigns_unique_ids_and_persists` |
| `remove_habit` removes only the targeted habit                   | `remove_habit_only_removes_the_targeted_habit`, `remove_habit_returns_true_on_hit_false_on_miss` |
| `toggle_completion` idempotent (toggle twice = original state)   | `toggle_completion_is_idempotent` |
| Completions on different dates accumulate                        | `completions_on_distinct_dates_accumulate` |
| `current_streak` = 0 for empty                                   | `current_streak_zero_with_no_completions`, `current_streak_zero_when_today_missed` |
| `current_streak` = N for N consecutive days ending today         | `current_streak_consecutive_days` |
| `current_streak` breaks when a day is missed                     | `current_streak_breaks_on_gap` |
| save → load round-trip preserves all data                        | `save_and_load_round_trip`, `load_from_missing_path_returns_empty_store` |

Bonus coverage retained from the core layer's own tests: `is_due` for Daily / Weekly / EveryNDays window boundaries and `longest_streak` over multi-run histories.

All storage tests use `tempfile::tempdir()` and call `storage::save_to` / `storage::load_from` against the temp path — no test ever touches the real `$XDG_DATA_HOME/habit-tracker/habits.json`.

## Build verification

- `cargo build` — OK
- `cargo build --release` — OK (binary at `target/release/habit-tracker`)
- `cargo check --tests` — OK
- `cargo test` — 15/15 passing
- `cargo test --release` — 15/15 passing

Release-binary dynamic linkage (`ldd target/release/habit-tracker`):

```
linux-vdso.so.1
libgcc_s.so.1 => /usr/lib/libgcc_s.so.1
libc.so.6     => /usr/lib/libc.so.6
/lib64/ld-linux-x86-64.so.2
```

Maps to Arch packages `gcc-libs` and `glibc`, both declared in `depends=()` in PKGBUILD.

## TUI compatibility checks

These were verified by reading `src/tui/app.rs` and `src/tui/views.rs` — the TUI itself requires a real TTY and cannot be exercised by this non-interactive harness.

- **Terminal restoration on panic.** `run_app` installs a panic hook (`install_panic_hook` at `src/tui/app.rs:299`) that calls `restore_terminal()` (LeaveAlternateScreen + disable_raw_mode) before chaining to the previous hook.
- **Terminal restoration on normal exit / error.** `TerminalGuard` (`src/tui/app.rs:267`) is an RAII guard whose `Drop` impl invokes `restore_terminal()`. Fires on quit, on `?` early-returns, and on panic unwind, so the parent shell is always restored.
- **Ctrl-C is a clean quit.** `App::handle_key` (`src/tui/app.rs:127`) checks `KeyModifiers::CONTROL + 'c'` first, regardless of the active screen, and sets `should_quit = true`.
- **Small terminal handling.** The event loop (`src/tui/app.rs:328`) measures `f.area()` each frame and falls back to `views::render_resize_notice` when `width < 80` or `height < 24`. Input is still polled, so Ctrl-C still quits.
- **Key-event filtering.** `next_key` (`src/tui/events.rs:10`) only surfaces `KeyEventKind::Press` events with a 250 ms poll, so key release/repeat events on Windows-style terminals don't double-fire actions.
- **TTY requirement (not a bug).** As tui-dev flagged: the binary needs a real TTY. If stdin/stdout are redirected, `crossterm` returns "No such device or address" and the program exits cleanly. Inherent crossterm/raw-mode constraint, not a regression.
- **Atomic save on exit.** `main` calls `storage::save` after `tui::run_app` returns, and `storage::save_to` writes to a `.tmp` sibling and renames into place — interrupted writes cannot corrupt the data file.

## namcap

`namcap PKGBUILD` was **skipped — namcap is not installed** on the build host (`command -v namcap` returns nothing). The PKGBUILD was hand-audited against Arch packaging guidelines instead:

- `pkgname` lower-case alphanumeric ✓
- `pkgver` matches `Cargo.toml` (`0.1.0`) ✓
- `pkgrel=1` ✓
- `arch=('x86_64')` ✓
- `license=('MIT')` ✓ (LICENSE file present at repo root)
- `makedepends=('cargo')` ✓ (rust toolchain comes via cargo)
- `depends=('gcc-libs' 'glibc')` matches actual dynamic linkage ✓
- `build()` uses `--release --locked`, separate `CARGO_TARGET_DIR` ✓
- `check()` runs `cargo test --release --locked` ✓
- `package()` uses `install -Dm755` for the binary and `install -Dm644` for LICENSE under `/usr/share/licenses/$pkgname/` ✓

## Final verdict

**PASS.** Tests are green in debug and release, the release binary builds cleanly, runtime dependencies match declarations, and the TUI's terminal-restoration paths (RAII guard + panic hook + Ctrl-C handling) all trace through cleanly. The PKGBUILD is ready for `makepkg -si` from the repo root.
