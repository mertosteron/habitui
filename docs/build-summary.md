# habit-tracker — Build Summary

A small terminal habit tracker written in Rust. Lets you add habits, mark them done each day, and visualise streaks in a calendar-style heatmap.

## What was built

- **Daily / weekly / every-N-days habits.** Frequency is per-habit; the streak math respects the chosen window.
- **One-keypress completion toggle.** Press `space` on the list to mark today done (or undo it).
- **Streak tracking.** Current streak (anchored at today) and longest streak (across all history) are shown on the detail view.
- **12-week heatmap.** Per-habit detail view renders a 12 × 7 grid of completion cells with a legend, similar to GitHub's contribution graph.
- **Persistent JSON storage.** State lives at `$XDG_DATA_HOME/habit-tracker/habits.json` (Linux) and is written atomically (write-to-`.tmp`-then-rename) on exit.
- **Crash-safe TUI.** A panic hook plus an RAII terminal guard guarantee the parent shell is restored to cooked mode even if the program panics mid-render. Ctrl-C is a global clean quit.
- **15 integration tests** covering the storage round-trip and all the streak/toggle/window edge cases.
- **Arch PKGBUILD** at the repo root that builds in-tree from the working directory and runs the test suite during `check()`.

## Architecture

### Crate layout

```
habit-tracker (Cargo workspace root)
├── Cargo.toml          — package + lib + bin definitions
├── PKGBUILD            — Arch package recipe (in-tree build)
├── LICENSE             — MIT
├── src/
│   ├── lib.rs          — re-exports the public API
│   ├── main.rs         — load → run TUI → save
│   ├── data.rs         — Frequency / Habit / HabitStore + streak logic
│   ├── storage.rs      — atomic JSON load/save + XDG path resolution
│   └── tui/
│       ├── mod.rs      — module wiring, re-exports run_app
│       ├── app.rs      — App state machine, key handling, panic hook, RAII guard
│       ├── views.rs    — list, add-form modal, detail/heatmap, confirm-delete
│       └── events.rs   — keyboard polling (Press-only, 250 ms tick)
├── tests/core_tests.rs — 15 integration tests against the public lib API
└── docs/build-summary.md
```

### Data model (`src/data.rs`)

- `Frequency` — `Daily | Weekly | EveryNDays(u32)`.
- `Habit { id, name, frequency, created_at, completions: BTreeSet<NaiveDate> }`. Completions are stored as a sorted set so range queries (and JSON round-trips) are deterministic.
- `HabitStore { version, habits: Vec<Habit>, next_id }`. `version` is reserved for future migrations; `next_id` is monotonic and `saturating_add`'d on each `add_habit` so id reuse is impossible.
- Streak math is window-based, not "consecutive-day" based, so weekly and every-N-days habits get treated correctly: a streak counts consecutive *windows* (each ending at `today`, `today − period`, …) that contain at least one completion.

### Storage (`src/storage.rs`)

- Default path: `dirs::data_dir().join("habit-tracker").join("habits.json")` — on Linux this is `$XDG_DATA_HOME/habit-tracker/habits.json` (falling back to `~/.local/share`).
- `save_to(store, path)` writes pretty-printed JSON to a sibling `<file>.tmp`, `fsync`s it, then renames into place — interrupted writes cannot corrupt an existing file.
- `load_from(path)` returns an empty `HabitStore` if the file is missing (so first run "just works"), and an `InvalidData` error if the JSON is malformed.

### TUI (`src/tui/`)

- **State machine** in `app.rs`: `Screen` is one of `List`, `AddHabit(form)`, `Detail { habit_id }`, `ConfirmDelete { habit_id }`. `App::handle_key` routes input to per-screen handlers.
- **Backend.** `ratatui 0.28` with `crossterm 0.28`. The terminal is set up via a `TerminalGuard` RAII handle that enters raw mode + the alternate screen on construction and restores both on `Drop`.
- **Panic safety.** `install_panic_hook` (called from `run_app`) wraps the existing hook so any panic restores the terminal *before* the backtrace prints. This means a panic in `views::render` won't leave the user's shell stuck on the alt screen.
- **Min terminal size.** The event loop checks `f.area()` each frame and renders a "resize to at least 80×24" notice when the terminal is smaller. Input is still polled in that mode, so Ctrl-C / `q` still quit.
- **Key polling.** `next_key()` uses a 250 ms `event::poll` and only surfaces `KeyEventKind::Press` events (filtering out release/repeat to avoid double-firing on Windows-style terminals).

### Persistence flow

```
main()
  ├── storage::load()   → HabitStore (or empty on first run)
  ├── tui::run_app(&mut store)
  │      ├── install_panic_hook()
  │      ├── TerminalGuard::enter()      (raw + alt screen)
  │      ├── event_loop until should_quit
  │      └── guard drops → terminal restored
  └── storage::save(&store)              (atomic write)
```

## Usage

### Run from source

```sh
cargo run --release
```

### Build the release binary

```sh
cargo build --release
./target/release/habit-tracker
```

### Run tests

```sh
cargo test
# or, matching what the PKGBUILD's check() runs:
cargo test --release --locked
```

### Install via PKGBUILD (Arch / Arch-derived distros)

From the repo root:

```sh
makepkg -si
```

This will:
1. Mirror `Cargo.toml`, `Cargo.lock`, `src/`, `tests/`, and `LICENSE` into the build sandbox.
2. Run `cargo build --release --locked`.
3. Run `cargo test --release --locked`.
4. Install `target/release/habit-tracker` to `/usr/bin/habit-tracker` and the MIT LICENSE to `/usr/share/licenses/habit-tracker/`.

After install, just run:

```sh
habit-tracker
```

### Key bindings

**List view** (default)
- `j` / `Down` — move selection down
- `k` / `Up` — move selection up
- `space` — toggle today's completion for the highlighted habit
- `a` — open the "add habit" modal
- `d` — delete highlighted habit (asks `y/n` to confirm)
- `g` or `Enter` — open the heatmap detail view for the highlighted habit
- `q` — quit (saves on exit)
- `Ctrl-C` — quit anywhere, any time

**Add habit modal**
- type to enter the habit name
- `Tab` — cycle the focused field (Name → Frequency → [N value if applicable])
- `space` / `←` / `→` — cycle Daily / Weekly / Every-N-days when the Frequency field is focused
- digits `0`–`9` — edit the N value when EveryNDays is selected and the N field is focused
- `Backspace` — delete a character (Name field) or shrink the N value (N field)
- `Enter` — save the habit
- `Esc` — cancel and return to the list

**Detail / heatmap view**
- `Esc` / `q` / `Enter` / `g` — return to the list

**Confirm-delete prompt**
- `y` — confirm delete
- `n` / `Esc` — cancel

## Storage location

| OS      | Path                                                     |
|---------|----------------------------------------------------------|
| Linux   | `$XDG_DATA_HOME/habit-tracker/habits.json` (default `~/.local/share/habit-tracker/habits.json`) |
| macOS   | `~/Library/Application Support/habit-tracker/habits.json` |
| Windows | `%APPDATA%\habit-tracker\habits.json`                    |

Resolved by the `dirs` crate; first run creates the directory automatically. The file is plain JSON — safe to back up, diff, or hand-edit.
