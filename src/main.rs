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

#[derive(Debug)]
struct Process {
    pid: usize,
    name: String,
}

fn main() -> io::Result<()> {
    tui::tui()
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
        0
    }
}

fn get_priority_processes() -> HashSet<usize> {
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line (std::in)");

    input
        .trim()
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|s| s.parse::<usize>().ok())
        .collect()
}

#[cfg(target_os = "windows")]
fn direct_process(process: &Process, mask: usize, log: &mut Vec<String>) {
    // Pins `process` to every core set in `mask` (bit i = core i) with a single
    // syscall. SetProcessAffinityMask *replaces* the mask rather than adding to
    // it, so calling this once per core (the old approach) only ever left the
    // process pinned to the last core in the loop.
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winbase::SetProcessAffinityMask;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::winnt::PROCESS_SET_INFORMATION;
    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION, 0, process.pid as u32);
        if handle.is_null() {
            log.push(format!("could not open process {}", process.pid));
            return;
        }
        let result = SetProcessAffinityMask(handle, mask.try_into().unwrap());
        CloseHandle(handle);
        if result == 0 {
            log.push(format!("could not set affinity for process {}", process.pid));
        } else {
            log.push(format!("Successfully pinned PID {} to mask {:#x}", process.pid, mask));
        }
    }
}

#[cfg(target_os = "linux")]
fn direct_process(process: &Process, mask: usize, log: &mut Vec<String>) {
    // Pins `process` to every core set in `mask` (bit i = core i) with a single
    // sched_setaffinity call instead of one call per core.
    use nix::sched::{CpuSet, sched_setaffinity};
    use nix::unistd::Pid;
    let mut cpu_set = CpuSet::new();
    for core in 0..usize::BITS as usize {
        if mask & (1 << core) != 0 {
            let _ = cpu_set.set(core);
        }
    }
    match sched_setaffinity(Pid::from_raw(process.pid as i32), &cpu_set) {
        Ok(_) => log.push(format!("Successfully pinned PID {} to mask {:#x}", process.pid, mask)),
        Err(e) => log.push(format!("Failed to pin PID {}: {}", process.pid, e)),
    }
}

fn eco_mode(log: &mut Vec<String>) {
    // Puts dispatch into eco mode by restricting every process to the first
    // half of cores. One affinity call per process (mask covering all eco
    // cores at once) instead of one call per core per process — the latter
    // did (processes * cores) OpenProcess/SetAffinity/CloseHandle round trips
    // to achieve the same result, since each call just overwrote the last.
    let processes = read_tasks();
    let core_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let eco_cores = (core_count + 1) / 2;
    let mask: usize = (0..eco_cores).fold(0, |m, c| m | (1 << c));
    for (_pid, process) in &processes {
        direct_process(process, mask, log);
    }
}

fn performance_mode(priority: &HashSet<usize>, log: &mut Vec<String>) {
    // this fn puts dispatch into performance mode [idk what to say lol]
    // will direct priority processes first then will direct based on mem_process()
    let processes = read_tasks();
    let sys = refreshed_system();
    let core_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    // Priority processes get unrestricted affinity (every core) in one call;
    // looping SetProcessAffinityMask per core only ever left them pinned to
    // whichever core happened to be last in the loop.
    let all_cores: usize = (0..core_count).fold(0, |m, c| m | (1 << c));

    for (pid, process) in &processes {
        if priority.contains(pid) {
            direct_process(process, all_cores, log);
        }
    }

    let mut ranked: Vec<&Process> = processes
        .values()
        .filter(|process| !priority.contains(&process.pid))
        .collect();
    ranked.sort_by_key(|process| {
        std::cmp::Reverse(score_process(&sys, priority, Pid::from(process.pid)))
    });

    for (i, process) in ranked.into_iter().enumerate() {
        direct_process(process, 1 << (i % core_count), log);
    }
}

// Returns CPU usage percentage for the given pid. Can exceed 100 for multi-threaded
// processes (sysinfo reports usage per logical core). `sys` must already have two
// refreshes spaced MIN_CPU_REFRESH_GAP apart (see `refreshed_system`) or this is 0.0.
fn cpu_usage_of(sys: &System, pid: Pid) -> f32 {
    if let Some(process) = sys.process(pid) {
        process.cpu_usage()
    } else {
        0.0
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
// process always outranks an unflagged one, however heavy). `sys` must come from
// `refreshed_system` (or two refreshes spaced MIN_CPU_REFRESH_GAP apart) for the
// cpu term to be meaningful.
fn score_process(sys: &System, priority: &HashSet<usize>, pid: Pid) -> u32 {
    if sys.process(pid).is_none() {
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
// to score many pids (performance_mode, ...) should build one
// `refreshed_system()` + priority set and call `score_process` directly instead of
// paying the refresh/sleep cost per pid.
fn eval_process(pid: Pid) -> u32 {
    let sys = refreshed_system();
    let priority = HashSet::new();
    return score_process(&sys, &priority, pid);
}

// This fn redistributes all processes across cores to balance load evenly.
fn auto_balance(log: &mut Vec<String>) {
    let processes = read_tasks();
    let core_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let all_cores: usize = (0..core_count).fold(0, |m, c| m | (1 << c));

    for (_pid, process) in &processes {
        direct_process(process, all_cores, log);
    }
}

// Reserves dedicated cores for a single high-priority process and pushes everything else away.
fn gaming_mode(target_pid: Pid, log: &mut Vec<String>) {
    let processes = read_tasks();
    let core_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let target = usize::from(target_pid);

    if let Some(process) = processes.get(&target) {
        let reserved: usize = (1..core_count).fold(0, |m, c| m | (1 << c));
        direct_process(process, reserved, log);
    }

    for (pid, process) in &processes {
        if *pid != target {
            direct_process(process, 1, log);
        }
    }
}

// Terminates a single process by pid (used by the TUI's process-select-and-kill action).
fn kill_process(pid: Pid, log: &mut Vec<String>) {
    let mut sys = System::new_all();
    sys.refresh_all();

    if let Some(process) = sys.process(pid) {
        let name = process.name().to_string();
        if process.kill() {
            log.push(format!("Killed PID {} ({})", pid, name));
        } else {
            log.push(format!("Failed to kill PID {} ({})", pid, name));
        }
    } else {
        log.push(format!("No such process: PID {}", pid));
    }
}

// Suspends (pauses) a process without killing it.
#[cfg(target_os = "windows")]
fn suspend_process(pid: Pid) {
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::winnt::{HANDLE, PROCESS_SUSPEND_RESUME};

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtSuspendProcess(process_handle: HANDLE) -> i32;
    }

    unsafe {
        let handle = OpenProcess(PROCESS_SUSPEND_RESUME, 0, usize::from(pid) as u32);
        if handle.is_null() {
            println!("could not open process {}", pid);
            return;
        }
        let status = NtSuspendProcess(handle);
        CloseHandle(handle);
        if status == 0 {
            println!("Successfully suspended PID {}", pid);
        } else {
            println!("could not suspend process {}", pid);
        }
    }
}

#[cfg(target_os = "linux")]
fn suspend_process(pid: Pid) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    match kill(Pid::from_raw(usize::from(pid) as i32), Signal::SIGSTOP) {
        Ok(_) => println!("Successfully suspended PID {}", pid),
        Err(e) => println!("Failed to suspend PID {}: {}", pid, e),
    }
}

// Resumes a previously suspended process.
#[cfg(target_os = "windows")]
fn resume_process(pid: Pid) {
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::winnt::{HANDLE, PROCESS_SUSPEND_RESUME};

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtResumeProcess(process_handle: HANDLE) -> i32;
    }

    unsafe {
        let handle = OpenProcess(PROCESS_SUSPEND_RESUME, 0, usize::from(pid) as u32);
        if handle.is_null() {
            println!("could not open process {}", pid);
            return;
        }
        let status = NtResumeProcess(handle);
        CloseHandle(handle);
        if status == 0 {
            println!("Successfully resumed PID {}", pid);
        } else {
            println!("could not resume process {}", pid);
        }
    }
}

#[cfg(target_os = "linux")]
fn resume_process(pid: Pid) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    match kill(Pid::from_raw(usize::from(pid) as i32), Signal::SIGCONT) {
        Ok(_) => println!("Successfully resumed PID {}", pid),
        Err(e) => println!("Failed to resume PID {}: {}", pid, e),
    }
}

// Reads a config file at `path` for user-defined priority rules and blacklists.
fn load_config(path: &str) -> (HashSet<usize>, HashSet<String>) {
    let mut priority = HashSet::new();
    let mut blacklist = HashSet::new();

    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(e) => {
            println!("failed to read config at {}: {}", path, e);
            return (priority, blacklist);
        }
    };

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line.split_once('=') {
            Some((key, value)) => match key.trim() {
                "priority" => {
                    if let Ok(pid) = value.trim().parse::<usize>() {
                        priority.insert(pid);
                    }
                }
                "blacklist" => {
                    blacklist.insert(value.trim().to_string());
                }
                other => println!("unrecognized config key: {}", other),
            },
            None => println!("unrecognized config line: {}", line),
        }
    }

    (priority, blacklist)
}

// Polls every `interval_secs` seconds, re-evaluating and re-pinning all processes.
fn watch(interval_secs: u64) {
    loop {
        let mut log = Vec::new();
        auto_balance(&mut log);
        for line in log {
            println!("{}", line);
        }
        std::thread::sleep(Duration::from_secs(interval_secs));
    }
}

#[cfg(target_os = "windows")]
fn current_affinity(pid: Pid) -> Vec<usize> {
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winbase::GetProcessAffinityMask;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::winnt::PROCESS_QUERY_INFORMATION;

    let mut cores = Vec::new();
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, usize::from(pid) as u32);
        if handle.is_null() {
            return cores;
        }
        let mut process_mask: usize = 0;
        let mut system_mask: usize = 0;
        let ok = GetProcessAffinityMask(handle, &mut process_mask, &mut system_mask);
        CloseHandle(handle);
        if ok != 0 {
            for core in 0..usize::BITS as usize {
                if process_mask & (1 << core) != 0 {
                    cores.push(core);
                }
            }
        }
    }
    cores
}

#[cfg(target_os = "linux")]
fn current_affinity(pid: Pid) -> Vec<usize> {
    use nix::sched::{sched_getaffinity, CpuSet};
    use nix::unistd::Pid;

    let mut cores = Vec::new();
    if let Ok(cpu_set) = sched_getaffinity(Pid::from_raw(usize::from(pid) as i32)) {
        for core in 0..CpuSet::count() {
            if cpu_set.is_set(core).unwrap_or(false) {
                cores.push(core);
            }
        }
    }
    cores
}

// Writes a snapshot of current process-to-core assignments to a log file.
fn log_state(path: &str) {
    let processes = read_tasks();
    let mut file = match std::fs::File::create(path) {
        Ok(file) => file,
        Err(e) => {
            println!("failed to open log file {}: {}", path, e);
            return;
        }
    };

    for (pid, process) in &processes {
        let cores = current_affinity(Pid::from(*pid));
        let _ = writeln!(file, "{} ({}): cores {:?}", pid, process.name, cores);
    }
}
