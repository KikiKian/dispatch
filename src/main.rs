use sysinfo::{System, SystemExt, ProcessExt, Pid};
use std::collections::HashMap;

mod tui;

#[derive(Debug)]
struct Process {
    pid: usize,
    name: String,
}

fn main() {
    //TODO link all of it together
    println!("temp");

    let tasks = read_tasks();
    println!("tasks {:#?}", tasks);
    let task0_pid = rand_pid(tasks);
    println!("tasks 0 : {}", task0_pid);
    let eval = eval_process(task0_pid.into());
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

fn eval_process(pid: Pid) -> u16 {
    //TODO write this fn
    //this fn will evauluate the stated proccess to see if it is a high priority process
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

fn get_priority_processes() -> HashMap<usize, Process> {
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
    // will direct priority processes first then will direct based on eval_process()
    let processes = read_tasks();
    let mut sys = System::new_all();
    sys.refresh_all();
    let _core_count = sys.physical_core_count().unwrap_or(1);

    let _priority = get_priority_processes();
}
