# Habitui

A fast, minimalist, and fully keyboard-driven habit tracker for your terminal, written in Rust. 

Whether you are trying to build a new positive routine or break an old bad habit, Habitui helps you stay on track. Watch your streaks grow and stay motivated with satisfying, terminal-native visual graphs.

## Features

- **Minimalist & Fast:** Built with Rust for peak performance and zero bloat.
- **100% Keyboard Controlled:** Navigate and manage everything without ever touching your mouse.
- **Build & Break:** Designed flexibly to track goals for both creating good habits and quitting bad ones.
- **Streak Tracking:** Keep the momentum going. The longer you stick to it, the higher your streak.
- **Visual Graphs:** Satisfying and clean charts rendered directly in your terminal to visualize your progress.

## Installation

Make sure you have [Rust and Cargo](https://rustup.rs/) installed.

```bash
git clone [https://github.com/yourusername/habitui.git](https://github.com/yourusername/habitui.git)
cd habitui
cargo build --release
You can find the compiled binary in target/release/. Optionally, move it to your local bin directory:

Bash
mv target/release/habitui ~/.local/bin/
Usage
Simply run the application from your terminal:

Bash
habitui
Default Keybindings
↑ / ↓ or k / j: Navigate through your habits

Enter / Space: Mark a habit as done/failed for the day

n / a: Add a new habit

d: Delete the selected habit

q / Esc: Quit the application

Contributing
Contributions, issues, and feature requests are welcome! Feel free to check the issues page.

License
This project is licensed under the MIT License - see the LICENSE file for details.
