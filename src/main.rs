use sysinfo::{System, SystemExt, ProcessExt};
use std::collections::HashMap;

struct Process {
    pid: String,
    name: String,
}

fn main() {
    println!("Hello, world!");
}

fn read_tasks() -> HashMap<String, Process> {
    let mut sys = System::new_all();
    sys.refresh_all();
    let mut all_processes: HashMap<String, Process> = HashMap::new();

    for (pid, process) in sys.processes() {
        let p = Process {
            pid: pid.to_string(),
            name: process.name().to_string(),
        };
        all_processes.insert(pid.to_string(), p);
    }

    all_processes
}
