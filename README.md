# Habitui

A fast, minimalist, and fully keyboard-driven habit tracker for your terminal, written in Rust.

Build positive routines or quit bad ones. Watch your streaks grow with circle-based progress strips, color-graded streak milestones, and a satisfying terminal-native UI.

<img width="920" height="490" alt="habitui_image1" src="https://github.com/user-attachments/assets/879a3e83-1674-443f-aed3-657d85fbdcef" />

## Features

- **Minimalist & fast** — written in Rust, zero bloat, instant launch.
- **100% keyboard controlled** — never touch your mouse.
- **Build & quit habits** — track goals to start, or to stop.
- **Flexible frequencies** — daily, every-N-days, or N-times-per-week. Streaks count calendar days; missing a quota period only breaks the streak when the period ends.
- **Streak milestones** — the streak number changes color as it climbs (week, month, quarter, year).
- **Themes** — cycle through Green / Blue / Red / Yellow palettes with `C`. Your choice persists.
- **Visual graphs** — 30-day progress strip, year activity row, week-by-week binary calendar, global heatmap.

## Install

You need [Rust + Cargo](https://rustup.rs/).

### Quick install (recommended for end users)

```bash
cargo install --git https://github.com/mertosteron/habitui
```

This builds and drops a `habitui` binary into `~/.cargo/bin/`. Make sure that directory is on your `$PATH` (Cargo's installer normally handles this).

### From a local clone

```bash
git clone https://github.com/mertosteron/habitui
cd habitui
cargo install --path .
```

### Build only (no install)

```bash
cargo build --release
./target/release/habitui
```

## Usage

```bash
habitui
```

Your data lives at:

| OS      | Path                                                  |
| ------- | ----------------------------------------------------- |
| Linux   | `~/.local/share/habitui/{habits,config}.json`         |
| macOS   | `~/Library/Application Support/habitui/`              |
| Windows | `%APPDATA%\habitui\`                                  |

## Keybindings

### List screen
| Key             | Action                                  |
| --------------- | --------------------------------------- |
| `↑ ↓` or `k j`  | Navigate habits                          |
| `Space`         | Toggle today's completion (Build habit) |
| `F`             | Log/clear today's failure (Quit habit)  |
| `Enter`         | Open habit detail view                  |
| `A`             | Add a new habit                         |
| `E`             | Edit the selected habit                 |
| `D`             | Delete the selected habit               |
| `G`             | Open the global heatmap                 |
| `C`             | Cycle theme (Green → Blue → Red → Yellow) |
| `[` / `]`       | Previous / next year                    |
| `Q` / `Esc`     | Quit                                    |

### Detail view
| Key            | Action                              |
| -------------- | ----------------------------------- |
| `E`            | Enter / exit edit mode              |
| `← → ↑ ↓` (in edit mode) | Move the calendar cursor   |
| `Space`        | Toggle completion on cursor day     |
| `[` / `]`      | Previous / next year                |
| `Esc` / `Q` / `Enter` | Back to list                  |

## Development

If you're hacking on the code and want `habitui` on your `$PATH` to always reflect your local edits, install the dev shim once:

```bash
./scripts/dev-link.sh
```

This drops a shell wrapper at `~/.cargo/bin/habitui` that runs `cargo run --release` against your source. Every edit → next launch rebuilds incrementally and runs the fresh binary. To go back to a static binary:

```bash
rm ~/.cargo/bin/habitui
cargo install --path .
```

### Running tests

```bash
cargo test
```

## Project layout

```
habitui/
├── Cargo.toml
├── src/
│   ├── main.rs        # entry point
│   ├── lib.rs         # public re-exports
│   ├── data.rs        # Habit, HabitStore, streak math
│   ├── storage.rs     # atomic load/save of habits.json
│   ├── config.rs      # Theme + persistent config.json
│   └── tui/
│       ├── mod.rs
│       ├── app.rs     # App state, key handling, event loop
│       ├── events.rs  # crossterm key polling
│       └── views.rs   # all rendering (list, detail, forms, global)
├── tests/             # integration tests for data + storage
└── scripts/
    └── dev-link.sh    # install local dev shim onto PATH
```

## Contributing

Issues and PRs welcome. Open an issue first if you want to discuss a larger change.

## License

MIT — see [LICENSE](LICENSE).
