use sysinfo::{System, SystemExt, ProcessExt};
use std::collections::HashMap;

struct Process {
    pid: String,
    name: String,
}

fn main() {
    println!("temp");
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

#[cfg(target_os = "windows")]
fn direct_process(process: &Process, core_id: usize) {
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winbase::SetProcessAffinityMask;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::winnt::PROCESS_ALL_ACCESS;

    let pid: u32 = process.pid.trim().parse().expect("invalid pid");
    let mask: usize = 1 << core_id;
    unsafe {
        let handle = OpenProcess(PROCESS_ALL_ACCESS, 0, pid);
        if handle.is_null() {
            println!("could not open process {}", pid);
            return;
        }
        let result = SetProcessAffinityMask(handle, mask.try_into().unwrap());
        CloseHandle(handle);
        if result == 0 {
            println!("could not set affinity for process {}", pid);
        } else {
            println!("Successfully pinned PID {} to core {}", pid, core_id);
        }
    }
}

#[cfg(target_os = "linux")]
fn direct_process(process: &Process, core_id: usize) {
    use nix::sched::{CpuSet, sched_setaffinity};
    use nix::unistd::Pid;

    let pid: i32 = process.pid.trim().parse().expect("invalid pid");
    let mut cpu_set = CpuSet::new();
    cpu_set.set(core_id).expect("invalid core id");
    match sched_setaffinity(Pid::from_raw(pid), &cpu_set) {
        Ok(_) => println!("Successfully pinned PID {} to core {}", pid, core_id),
        Err(e) => println!("Failed to pin PID {}: {}", pid, e),
    }
}
