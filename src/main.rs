use sysinfo::{System, SystemExt, ProcessExt};
use std::collections::HashMap;

mod tui;

struct Process {
    pid: String,
    name: String,
}

fn main() {
    //TODO link all of it together
    println!("temp");
}

fn read_tasks() -> HashMap<String, Process> {
    // reads current processes sys has 

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

fn eval_process() -> u16 {
    //TODO write this fn 
    //this fn will evauluate the stated proccess to see if it is a high priority process
    //temp
    return 0 
}

fn get_priority_processes() -> HashMap<String, Process>{
    //TODO
    todo!()
}

#[cfg(target_os = "windows")]
fn direct_process(process: &Process, core_id: usize) {
    // this fn directs the stated process to designated core_id (ex process -> core 5)

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
    // this fn directs the stated process to designated core_id (ex process -> core 5)

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

fn eco_mode() {
    // this fn puts dispatch into eco mode so that it uses less resources 
    let processes: HashMap<String, Process> = read_tasks();
    let mut sys = System::new_all();
    sys.refresh_all();
    let core_count = sys.physical_core_count().unwrap_or(1);
    let eco_cores = if core_count % 2 != 0 {
        (core_count + 1) / 2
    } else {
        core_count / 2
    };
    for (_pid, process) in &processes { 
        for core in 0..eco_cores {
            direct_process(process, core);
        }
    }
}

fn performance_mode() {
    //TODO write this fn 
    // this fn puts dispatch into performance mode [idk what to say lol]
    // will direct priority processes first then will direct based on eval_process()
    let processes: HashMap<String, Process> = read_tasks();
    let mut sys = System::new_all();
    sys.refresh_all();
    let core_count = sys.physical_core_count().unwrap_or(1);
    
    let _priority = get_priority_processes();
}
