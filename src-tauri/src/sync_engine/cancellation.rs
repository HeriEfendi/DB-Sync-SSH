use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub static USER_CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);
pub static ACTIVE_CHILD_PIDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

pub fn register_child_pid(pid: u32) {
    if let Ok(mut lock) = ACTIVE_CHILD_PIDS.lock() {
        if !lock.contains(&pid) {
            lock.push(pid);
        }
    }
}

pub fn unregister_child_pid(pid: u32) {
    if let Ok(mut lock) = ACTIVE_CHILD_PIDS.lock() {
        lock.retain(|&p| p != pid);
    }
}

pub fn kill_all_child_processes() {
    let pids = {
        if let Ok(mut lock) = ACTIVE_CHILD_PIDS.lock() {
            let p = lock.clone();
            lock.clear();
            p
        } else {
            Vec::new()
        }
    };
    for pid in pids {
        #[cfg(unix)]
        {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .output();
        }
        #[cfg(windows)]
        {
            let _ = crate::process_utils::new_std_command("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .output();
        }
    }
}

pub fn is_cancelled() -> bool {
    USER_CANCEL_REQUESTED.load(Ordering::SeqCst)
}

pub fn reset_cancel_state() {
    USER_CANCEL_REQUESTED.store(false, Ordering::SeqCst);
}

pub fn cancel_sync() {
    USER_CANCEL_REQUESTED.store(true, Ordering::SeqCst);
    kill_all_child_processes();
}

