use std::cmp::Ordering;

use sysinfo::{ProcessExt, System, SystemExt};

pub fn visualizer() {
    let mut sys = System::new_all();
    sys.refresh_all();

    let processes: Vec<(usize, String, f32, u64)> = sys
        .processes()
        .iter()
        .map(|(pid, process)| {
            let pid = usize::from(*pid);
            let name = process.name().to_string();
            let cpu = process.cpu_usage();
            let mem_mb = process.memory() / 1024;
            (pid, name, cpu, mem_mb)
        })
        .collect();

    if processes.is_empty() {
        println!("No processes available to visualize.");
        return;
    }

    let mut top_cpu = processes.clone();
    top_cpu.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(Ordering::Equal));

    let mut top_mem = processes;
    top_mem.sort_by(|a, b| b.3.cmp(&a.3));

    println!("Dispatch visualizer");
    println!("Total running processes: {}", top_mem.len());
    println!();
    print_process_section("Top CPU usage", &top_cpu, 10, |item| item.2, "CPU", |item| format!("{:.2}%", item.2));
    println!();
    print_process_section("Top memory usage", &top_mem, 10, |item| item.3 as f32, "MEM", |item| format!("{} MiB", item.3));
}

fn print_process_section<F, L>(
    title: &str,
    items: &[(usize, String, f32, u64)],
    limit: usize,
    metric: F,
    metric_label: &str,
    label_for: L,
) where
    F: Fn(&(usize, String, f32, u64)) -> f32,
    L: Fn(&(usize, String, f32, u64)) -> String,
{
    let max_value = items
        .iter()
        .map(|item| metric(item))
        .fold(0.0_f32, f32::max)
        .max(1.0);

    println!("{}", title);
    println!(" {:>6}  {:<30}  {:>10}  {}", "PID", "Name", metric_label, "Usage");
    println!("{:-<70}", "");

    for entry in items.iter().take(limit) {
        let value = metric(entry).max(0.0);
        let bar = render_bar(value, max_value, 24);
        let label = label_for(entry);

        println!(
            " {:>6}  {:<30}  {:>10}  {}",
            entry.0,
            truncate(&entry.1, 30),
            label,
            bar,
        );
    }
}

fn render_bar(value: f32, max: f32, width: usize) -> String {
    let filled = ((value / max).clamp(0.0, 1.0) * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), " ".repeat(empty))
}

fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        let mut truncated = text.chars().take(max_len - 1).collect::<String>();
        truncated.push('…');
        truncated
    }
}
