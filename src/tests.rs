#[cfg(test)]
mod tests {
    use crate::{Process, eval_process, rand_pid, read_tasks};
    use std::collections::HashMap;
    use sysinfo::Pid;

    // --- rand_pid ---

    #[test]
    fn test_rand_pid_returns_pid_from_map() {
        let mut processes = HashMap::new();
        processes.insert(1234, Process { pid: 1234, name: "foo".to_string() });
        processes.insert(5678, Process { pid: 5678, name: "bar".to_string() });
        let pid = rand_pid(processes);
        assert!(pid == 1234 || pid == 5678);
    }

    #[test]
    fn test_rand_pid_single_entry_returns_that_pid() {
        let mut processes = HashMap::new();
        processes.insert(42, Process { pid: 42, name: "only".to_string() });
        assert_eq!(rand_pid(processes), 42);
    }

    #[test]
    #[should_panic]
    fn test_rand_pid_panics_on_empty_map() {
        let processes: HashMap<usize, Process> = HashMap::new();
        rand_pid(processes);
    }

    // --- read_tasks ---

    #[test]
    fn test_read_tasks_not_empty() {
        let tasks = read_tasks();
        assert!(!tasks.is_empty(), "expected at least one running process");
    }

    #[test]
    fn test_read_tasks_pid_keys_match_process_pids() {
        let tasks = read_tasks();
        for (key, process) in &tasks {
            assert_eq!(*key, process.pid, "map key should equal Process.pid");
        }
    }

    #[test]
    fn test_read_tasks_process_names_nonempty() {
        let tasks = read_tasks();
        for process in tasks.values() {
            assert!(!process.name.is_empty(), "process name should not be empty");
        }
    }

    // --- eval_process ---

    #[test]
    fn test_eval_process_current_pid_has_memory() {
        let current_pid = std::process::id() as usize;
        let result = eval_process(Pid::from(current_pid));
        assert!(result > 0, "current process should report non-zero memory");
    }

    #[test]
    fn test_eval_process_nonexistent_pid_returns_zero() {
        // PID 999_999 is extremely unlikely to exist
        let result = eval_process(Pid::from(999_999_usize));
        assert_eq!(result, 0);
    }
}
