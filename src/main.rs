use habitui::{storage, tui, HabitStore};

fn main() -> std::io::Result<()> {
    let mut store = storage::load().unwrap_or_else(|_| HabitStore::new());
    let run_result = tui::run_app(&mut store);
    storage::save(&store)?;
    run_result
}
