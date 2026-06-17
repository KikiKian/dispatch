use sysinfo::{System, SystemExt, ProcessExt, Pid, ProcessRefreshKind};
use std::collections::HashMap;
use std::io::{self, Write};

mod tui;
mod tests;

#[derive(Debug)]
pub struct Process {
    pub pid: usize,
    pub name: String,
}

fn main() {
    println!("temp");

    let tasks = read_tasks();
    println!("tasks {:#?}", tasks);
    let task0_pid = rand_pid(tasks);
    println!("tasks 0 : {}", task0_pid);
    let eval = mem_process(task0_pid.into());
    println!("eval of task0 {}", eval);
}

pub fn read_tasks() -> HashMap<usize, Process> {
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



pub fn rand_pid(processes: HashMap<usize, Process>) -> usize {
    let first_pid = processes.keys().next().unwrap();
    return *first_pid;
}

pub fn mem_process(pid: Pid) -> u16 { 
    let mut sys = System::new_all();
    sys.refresh_all();

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

// Returns CPU usage percentage (0.0–100.0) for the given pid.
fn cpu_usage_of(pid: Pid) -> f32 {
    let mut sys = System::new_all();

    sys.refresh_processes_specifics(
        ProcessRefreshKind::new().with_cpu()
    );

    if let Some(process) = sys.process(pid) {
        let cpu_usage: f32 = process.cpu_usage();
        println!("Process '{}' is using {:.2}% CPU", process.name(), process.cpu_usage());
        return cpu_usage;
    } else {
        println!("Process with PID {} not found.", pid);
        return 0.0;
    }
}

// Combines CPU usage and memory into a single priority score (higher = more important).
fn eval_process(pid: Pid) -> u32 {
    let mem = mem_process(pid);
    let usage = cpu_usage_of(pid);
    
    todo!();
}

// Returns the load (0.0–1.0) for each logical core.
fn get_core_loads() -> Vec<f32> {
    todo!()
}

// Redistributes all processes across cores to balance load evenly.
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
