use sysinfo::{System, SystemExt, ProcessExt, Pid, ProcessRefreshKind};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::time::Duration;

const W_CPU: f64 = 1.0;
const W_MEM: f64 = 1.0;
const PRIORITY_BONUS: u32 = 1_000_000;

mod tui;
mod goodies;

#[derive(Debug)]
struct Process {
    pid: usize,
    name: String,
}

fn main() -> io::Result<()> {
    tui::tui()
}

fn processes_from(sys: &System) -> HashMap<usize, Process> {
    sys.processes()
        .iter()
        .map(|(pid, process)| {
            let pid = usize::from(*pid);
            (pid, Process { pid, name: process.name().to_string() })
        })
        .collect()
}

fn read_tasks() -> HashMap<usize, Process> {
    let sys = System::new_all();
    processes_from(&sys)
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
    let processes = read_tasks();
    let core_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let eco_cores = (core_count + 1) / 2;
    let mask: usize = (0..eco_cores).fold(0, |m, c| m | (1 << c));
    for (_pid, process) in &processes {
        direct_process(process, mask, log);
    }
}

fn performance_mode(priority: &HashSet<usize>, log: &mut Vec<String>) {
    let sys = refreshed_system();
    let processes = processes_from(&sys);
    let core_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);

    let has_priority = processes.keys().any(|pid| priority.contains(pid));
    let dedicated_cores = if has_priority { (core_count + 1) / 2 } else { 0 };
    let general_cores = core_count - dedicated_cores;
    let priority_mask: usize = (0..dedicated_cores).fold(0, |m, c| m | (1 << c));

    for (pid, process) in &processes {
        if priority.contains(pid) {
            direct_process(process, priority_mask, log);
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
        let mask = if general_cores == 0 {
            priority_mask
        } else {
            1 << (dedicated_cores + i % general_cores)
        };
        direct_process(process, mask, log);
    }
}

fn cpu_usage_of(sys: &System, pid: Pid) -> f32 {
    if let Some(process) = sys.process(pid) {
        process.cpu_usage()
    } else {
        0.0
    }
}

const MIN_CPU_REFRESH_GAP: Duration = Duration::from_millis(200);

fn refreshed_system() -> System {
    let mut sys = System::new_all();
    sys.refresh_processes_specifics(ProcessRefreshKind::new().with_cpu());
    std::thread::sleep(MIN_CPU_REFRESH_GAP);
    sys.refresh_all();
    sys
}

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

fn eval_process(pid: Pid) -> u32 {
    let sys = refreshed_system();
    let priority = HashSet::new();
    return score_process(&sys, &priority, pid);
}

fn auto_balance(log: &mut Vec<String>) {
    let processes = read_tasks();
    let core_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let all_cores: usize = (0..core_count).fold(0, |m, c| m | (1 << c));

    for (_pid, process) in &processes {
        direct_process(process, all_cores, log);
    }
}

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

fn kill_process(pid: Pid, log: &mut Vec<String>) {
    let sys = System::new_all();

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
