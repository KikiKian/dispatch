use sysinfo::{System, SystemExt, ProcessExt, Pid, ProcessRefreshKind};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::time::Duration;

// Equal weighting by default; tune these to favor cpu-bound or memory-bound activity.
const W_CPU: f64 = 1.0;
const W_MEM: f64 = 1.0;
// Resource scores top out around (100 + 100) * 100 = 20_000, so this dominates unconditionally.
const PRIORITY_BONUS: u32 = 1_000_000;

mod tui;
mod tests;

#[derive(Debug)]
struct Process {
    pid: usize,
    name: String,
}

fn main() {
    println!("temp");

    let tasks = read_tasks();
    println!("tasks {:#?}", tasks);
    let task0_pid = rand_pid(tasks);
    println!("tasks 0 : {}", task0_pid);
    let mut sys = System::new_all();
    sys.refresh_all();
    let eval = mem_process(&sys, task0_pid.into());
    println!("eval of task0 {}", eval);
}

fn read_tasks() -> HashMap<usize, Process> {
    // reads current processes sys has
    let mut sys = System::new_all();
    sys.refresh_all();
    sys.processes()
        .iter()
        .map(|(pid, process)| {
            let pid = usize::from(*pid);
            (pid, Process { pid, name: process.name().to_string() })
        })
        .collect()
}



fn rand_pid(processes: HashMap<usize, Process>) -> usize {
    let first_pid = processes.keys().next().unwrap();
    return *first_pid;
}

fn mem_process(sys: &System, pid: Pid) -> u16 {
    if let Some(process) = sys.process(pid) {
        let mem_kb = process.memory();
        let mem_mb = mem_kb / 1024;

        return mem_mb as u16;
    } else {
        println!("failiure retrieving eval for pid: {}", pid);
        return 0;
    }
}

fn get_priority_processes() -> usize {
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line (std::in)");
    return input.trim().parse::<usize>().unwrap_or(0);
}

#[cfg(target_os = "windows")]
fn direct_process(process: &Process, core_id: usize) {
    // this fn directs the stated process to designated core_id (ex process -> core 5)
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winbase::SetProcessAffinityMask;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::winnt::PROCESS_ALL_ACCESS;
    let mask: usize = 1 << core_id;
    unsafe {
        let handle = OpenProcess(PROCESS_ALL_ACCESS, 0, process.pid as u32);
        if handle.is_null() {
            println!("could not open process {}", process.pid);
            return;
        }
        let result = SetProcessAffinityMask(handle, mask.try_into().unwrap());
        CloseHandle(handle);
        if result == 0 {
            println!("could not set affinity for process {}", process.pid);
        } else {
            println!("Successfully pinned PID {} to core {}", process.pid, core_id);
        }
    }
}

#[cfg(target_os = "linux")]
fn direct_process(process: &Process, core_id: usize) {
    // this fn directs the stated process to designated core_id (ex process -> core 5)
    use nix::sched::{CpuSet, sched_setaffinity};
    use nix::unistd::Pid;
    let mut cpu_set = CpuSet::new();
    cpu_set.set(core_id).expect("invalid core id");
    match sched_setaffinity(Pid::from_raw(process.pid as i32), &cpu_set) {
        Ok(_) => println!("Successfully pinned PID {} to core {}", process.pid, core_id),
        Err(e) => println!("Failed to pin PID {}: {}", process.pid, e),
    }
}

fn eco_mode() {
    // this fn puts dispatch into eco mode so that it uses less resources
    let processes = read_tasks();
    let mut sys = System::new_all();
    sys.refresh_all();
    let core_count = sys.physical_core_count().unwrap_or(1);
    let eco_cores = (core_count + 1) / 2;
    for (_pid, process) in &processes {
        for core in 0..eco_cores {
            direct_process(process, core);
        }
    }
}

fn performance_mode() {
    //TODO write this fn
    // this fn puts dispatch into performance mode [idk what to say lol]
    // will direct priority processes first then will direct based on mem_process()
    let processes = read_tasks();
    let mut sys = System::new_all();
    sys.refresh_all();
    let _core_count = sys.physical_core_count().unwrap_or(1);

    let _priority = get_priority_processes();

    todo!();
}

// Returns CPU usage percentage for the given pid. Can exceed 100 for multi-threaded
// processes (sysinfo reports usage per logical core). `sys` must already have two
// refreshes spaced MIN_CPU_REFRESH_GAP apart (see `refreshed_system`) or this is 0.0.
fn cpu_usage_of(sys: &System, pid: Pid) -> f32 {
    if let Some(process) = sys.process(pid) {
        let cpu_usage: f32 = process.cpu_usage();
        println!("Process '{}' is using {:.2}% CPU", process.name(), cpu_usage);
        return cpu_usage;
    } else {
        println!("Process with PID {} not found.", pid);
        return 0.0;
    }
}

// sysinfo computes CPU usage as a delta between refreshes, so a freshly created
// System always reports 0.0 on its first reading. Refresh twice with a real gap
// in between to get a meaningful sample.
const MIN_CPU_REFRESH_GAP: Duration = Duration::from_millis(200);

fn refreshed_system() -> System {
    let mut sys = System::new_all();
    sys.refresh_processes_specifics(ProcessRefreshKind::new().with_cpu());
    std::thread::sleep(MIN_CPU_REFRESH_GAP);
    sys.refresh_all();
    sys
}

// Core scorer: combines normalized cpu/mem usage into a single 0..~20_000 score,
// or PRIORITY_BONUS outright if `pid` is in the user-flagged priority set (a flagged
// process always outranks an unflagged one, however heavy, so threshold-based logic
// like `kill_low_priority` can never touch it). `sys` must come from `refreshed_system`
// (or two refreshes spaced MIN_CPU_REFRESH_GAP apart) for the cpu term to be meaningful.
fn score_process(sys: &System, priority: &HashSet<usize>, pid: Pid) -> u32 {
    if sys.process(pid).is_none() {
        println!("failiure retrieving eval for pid: {}", pid);
        return 0;
    }

    if priority.contains(&usize::from(pid)) {
        return PRIORITY_BONUS;
    }

    let mem_mb = mem_process(sys, pid) as f64;
    let total_mem_mb = (sys.total_memory() / 1024).max(1) as f64;
    let mem_norm = (mem_mb / total_mem_mb * 100.0).min(100.0);

    let num_cpus = sys.cpus().len().max(1) as f64;
    let cpu_norm = (cpu_usage_of(sys, pid) as f64 / num_cpus).min(100.0);

    ((W_CPU * cpu_norm + W_MEM * mem_norm) * 100.0) as u32
}

// Combines CPU usage and memory into a single priority score (higher = more important).
// Convenience wrapper for one-off lookups (used by main/tests): builds its own
// CPU-ready System and scores with no priority pids flagged. Batch callers that need
// to score many pids (performance_mode, kill_low_priority, ...) should build one
// `refreshed_system()` + priority set and call `score_process` directly instead of
// paying the refresh/sleep cost per pid.
fn eval_process(pid: Pid) -> u32 {
    let sys = refreshed_system();
    let priority = HashSet::new();
    return score_process(&sys, &priority, pid);
}

// Returns the load (0.0–1.0) for each logical core.
fn get_core_loads() -> Vec<f32> {
    todo!()
}

// This fn redistributes all processes across cores to balance load evenly.
fn auto_balance() {
    todo!()
}

// Reserves dedicated cores for a single high-priority process and pushes everything else away.
fn gaming_mode(target_pid: Pid) {
    todo!()
}

// Terminates all processes whose score falls below `threshold`.
fn kill_low_priority(threshold: u32) {
    todo!()
}

// Suspends (pauses) a process without killing it.
fn suspend_process(pid: Pid) {
    todo!()
}

// Resumes a previously suspended process.
fn resume_process(pid: Pid) {
    todo!()
}

// Reads a config file at `path` for user-defined priority rules and blacklists.
fn load_config(path: &str) {
    todo!()
}

// Polls every `interval_secs` seconds, re-evaluating and re-pinning all processes.
fn watch(interval_secs: u64) {
    todo!()
}

// Writes a snapshot of current process-to-core assignments to a log file.
fn log_state(path: &str) {
    todo!()
}
