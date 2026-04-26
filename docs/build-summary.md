# habitui — Build Summary

A small terminal habit tracker written in Rust. Add habits, mark them done
each day (or log a slip on a habit you're trying to quit), and watch streaks
build up.

## What was built

- **Build *and* Quit habits.** Build habits (do this regularly) track
  completions; Quit habits (abstain from this) auto-increment a streak from
  `created_at` and reset it when you log a failure.
- **Four frequencies.** `Daily`, `Weekly` (rolling 7-day window),
  `EveryNDays(n)` (rolling n-day window), and `NTimesPerWeek(n)` (n
  completions per ISO week, Mon–Sun).
- **Edit a habit in place.** Change name and/or frequency without touching
  completions, failures, `created_at`, or `id`.
- **Streak tracking.** Current streak (anchored at today) and longest streak
  (across all history) are computed per-habit kind and per-frequency.
- **Persistent JSON storage with v1 → v2 forward compat.** Older v1 stores
  load cleanly: missing `kind` defaults to `Build` and the on-disk version
  is bumped to 2 on the next save.
- **Crash-safe TUI.** A panic hook plus an RAII terminal guard guarantee the
  parent shell is restored even if the program panics mid-render. Ctrl-C is
  a global clean quit.
- **31 integration tests** covering every storage and streak edge case
  including v1 → v2 migration.
- **Arch PKGBUILD** at the repo root that builds in-tree from the working
  directory and runs the test suite during `check()`.

## Architecture

### Crate layout

```
habitui (Cargo workspace root)
├── Cargo.toml              package + lib + bin definitions
├── PKGBUILD                Arch package recipe (in-tree build)
├── LICENSE                 MIT
├── src/
│   ├── lib.rs              re-exports the public API
│   ├── main.rs             load → run TUI → save
│   ├── data.rs             Frequency / HabitKind / Habit / HabitStore + streak logic
│   ├── storage.rs          atomic JSON load/save + XDG path resolution + v1→v2 migration
│   └── tui/                ratatui front-end
└── tests/core_tests.rs     31 integration tests against the public lib API
```

### Data model (`src/data.rs`)

- `Frequency` — `Daily | Weekly | EveryNDays(u32) | NTimesPerWeek(u32)`.
- `HabitKind` — `Build` (default) or `Quit { failures: BTreeSet<NaiveDate> }`.
  `Habit::kind` carries `#[serde(default)]` so v1 records (no `kind` field)
  deserialize as `Build`.
- `Habit { id, name, frequency, created_at, completions, kind }`. Completions
  are a `BTreeSet` so range queries and JSON round-trips are deterministic.
- `HabitStore { version, habits, next_id }`. `next_id` is monotonic and
  `saturating_add`'d, so id reuse is impossible.
- `HabitError` — `NotFound(id)` or `NotApplicable(&'static str)`.

#### Edit semantics

`HabitStore::edit_habit(id, Option<String>, Option<Frequency>)` updates name
and/or frequency in place. It never touches `completions`, `kind` failures,
`created_at`, or `id`. Passing `None`/`None` is a successful no-op. Unknown
id returns `Err(NotFound)`.

#### Quit-habit semantics

A Quit habit's streak auto-increments from `created_at`:
- Day-of-creation, no failures → streak = 1.
- N days after creation, no failures → streak = N + 1 (created day counts).
- On the day of a failure → streak = 0.
- Day after a failure → streak = 1; streak then continues to count days
  since that most-recent failure.
- `is_due` is always `false` for Quit (passive — nothing to "do today").
- `toggle_completion` returns `None` on Quit; use `log_failure` /
  `clear_failure` instead. Both are `Err(NotApplicable)` on Build.
- `is_complete_on` and `completions_in_range` alias to the failure set for
  Quit habits, so the heatmap can render slips with the same plumbing.

#### NTimesPerWeek semantics

ISO week boundaries (Mon..Sun) are computed by `iso_week_start(date)`:
`date - num_days_from_monday(weekday(date))`. The current week is "due"
until n completions are logged within that Mon..Sun. The current streak is
the count of consecutive past full weeks at quota, plus 1 if the current
week has already met quota; an under-quota current week does *not* break
the streak. The longest streak is the longest run of consecutive ISO weeks
between the first and last completion that each have ≥ n completions.

#### Store versioning

`STORE_VERSION = 2`. On `load_from`:
- v1 JSON (no `kind`, `version: 1`) deserializes via the serde default and
  the in-memory `version` is bumped to 2 so the next `save_to` writes the
  current schema.
- The on-disk path moved from `<data_dir>/habit-tracker/habits.json` to
  `<data_dir>/habitui/habits.json`. Pre-existing v1 data does not auto-
  migrate across the directory rename — users who care about history
  should `mv` the old file into the new location before first launch.

### Storage (`src/storage.rs`)

`save_to(store, path)` writes pretty JSON to a sibling `<file>.tmp`,
`fsync`s it, then renames into place — interrupted writes cannot corrupt
an existing file. `load_from(path)` returns an empty `HabitStore` if the
file is missing (so first run "just works") and `InvalidData` if JSON is
malformed.

## Usage

### Run from source

```sh
cargo run --release
```

### Build the release binary

```sh
cargo build --release --locked
./target/release/habitui
```

### Run tests

```sh
cargo test --release --locked
```

### Install via PKGBUILD (Arch / Arch-derived)

From the repo root:

```sh
makepkg -si
```

This mirrors `Cargo.toml`, `Cargo.lock`, `src/`, `tests/`, and `LICENSE`
into the build sandbox, runs `cargo build --release --locked`, runs
`cargo test --release --locked`, and installs `target/release/habitui` to
`/usr/bin/habitui` and the MIT LICENSE to
`/usr/share/licenses/habitui/LICENSE`.

After install, run:

```sh
habitui
```

### Key bindings (TUI)

The detailed keybinding set is owned by the TUI module — see `src/tui/` for
the authoritative source. Conventionally:

- List view: `j`/`k` move, `space` toggle today's completion (Build) /
  log-or-clear today's failure (Quit), `a` add, `e` edit, `d` delete, `g` /
  `Enter` open detail, `q` / `Ctrl-C` quit.
- Add / edit modal: type to edit name, `Tab` cycles fields, `space` /
  arrows cycle frequency variants, digits edit n values, `Enter` saves,
  `Esc` cancels.
- Detail view: `Esc` / `q` / `Enter` returns to the list.

### Storage location

| OS      | Path                                                          |
|---------|---------------------------------------------------------------|
| Linux   | `$XDG_DATA_HOME/habitui/habits.json` (default `~/.local/share/habitui/habits.json`) |
| macOS   | `~/Library/Application Support/habitui/habits.json`           |
| Windows | `%APPDATA%\habitui\habits.json`                               |

Resolved by the `dirs` crate; first run creates the directory automatically.

## Known limitations

- **TTY required.** The binary needs a real terminal. Redirected stdin/stdout
  causes `crossterm` to fail on raw-mode entry; the program exits cleanly
  but cannot run headless. Inherent to the TUI backend.
- **No automatic v1 directory migration.** v1 data at
  `<data_dir>/habit-tracker/habits.json` is not moved to
  `<data_dir>/habitui/habits.json` automatically — users have to `mv` it.
  The JSON itself is forward-compatible once relocated.
- **`Habit::longest_streak` for Quit habits is bounded by the data set, not
  by `today`.** The trailing open-ended span runs only up to the latest
  failure (or `created_at` if there are no failures); for an ongoing
  abstinence streak that already exceeds the historical best, callers
  should prefer `current_streak(today)`.
- **Single-user, single-machine.** No sync, no multi-device merge.
- **`NTimesPerWeek` weeks are ISO weeks (Mon..Sun)** with no per-user
  override. Not configurable.
