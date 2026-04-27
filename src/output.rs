use std::time::Duration;

use comfy_table::{Cell, Table, presets::UTF8_FULL};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

/// Creates and starts a spinner with the given message.
///
/// The spinner ticks every 80 ms using a braille animation rendered in cyan.
///
/// # Arguments
///
/// * `message` - The status text to display next to the spinner.
///
/// # Returns
///
/// Returns a running `ProgressBar` that must be explicitly stopped
/// (e.g., via `pb.finish_and_clear()`) when the operation completes.
pub fn spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

/// Renders a formatted UTF-8 table to stdout with the given headers and rows.
///
/// # Arguments
///
/// * `headers` - Column header labels.
/// * `rows` - Table rows, where each inner `Vec<String>` represents one row of cell values.
pub fn print_table(headers: Vec<&str>, rows: Vec<Vec<String>>) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(headers.iter().map(Cell::new));

    for row in rows {
        table.add_row(row.iter().map(Cell::new));
    }

    println!("{table}");
}


pub fn print_error(message: &str) {
    eprintln!("{} {}", "error:".red().bold(), message);
}

pub fn print_success(message: &str) {
    println!("{message}");
}
