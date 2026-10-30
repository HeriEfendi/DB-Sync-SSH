use std::path::{Path, PathBuf};

/// Helper utilities to construct child process commands with `CREATE_NO_WINDOW` on Windows.
/// On Windows, GUI applications spawning console processes (such as mysql, mariadb, docker, taskkill)
/// without CREATE_NO_WINDOW cause a new cmd.exe/conhost console window to open and flash.
/// This module ensures all child processes spawned by DB-Sync are executed silently in the background.

#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

#[inline]
pub fn new_std_command<S: AsRef<std::ffi::OsStr>>(program: S) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[inline]
pub fn new_tokio_command<S: AsRef<std::ffi::OsStr>>(program: S) -> tokio::process::Command {
    #[allow(unused_mut)]
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub fn resolve_mysql_binary(names: &[&str]) -> Result<PathBuf, String> {
    let executable_names: Vec<String> = names
        .iter()
        .map(|name| {
            #[cfg(windows)]
            {
                if name.to_ascii_lowercase().ends_with(".exe") {
                    name.to_string()
                } else {
                    format!("{}.exe", name)
                }
            }
            #[cfg(not(windows))]
            {
                name.to_string()
            }
        })
        .collect();

    #[cfg(unix)]
    if let Some(preferred) = executable_names.first().and_then(|name| {
        let path = Path::new("/usr/bin").join(name);
        path.is_file().then_some(path)
    }) {
        return Ok(preferred);
    }

    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            candidates.extend(executable_names.iter().map(|name| directory.join(name)));
        }
    }

    #[cfg(windows)]
    {
        for variable in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
            if let Some(program_files) = std::env::var_os(variable) {
                let program_files = PathBuf::from(program_files);
                for vendor in ["MySQL", "MariaDB"] {
                    if let Some(binary) =
                        find_mysql_binary_in_tree(&program_files.join(vendor), &executable_names, 2)
                    {
                        candidates.push(binary);
                    }
                }
            }
        }

        let system_drive = std::env::var_os("SystemDrive")
            .map(|drive| PathBuf::from(format!("{}\\", drive.to_string_lossy())))
            .unwrap_or_else(|| PathBuf::from("C:\\"));
        let known_roots = [
            system_drive.join("xampp/mysql/bin"),
            system_drive.join("laragon/bin/mysql"),
            system_drive.join("laragon/bin/mariadb"),
            system_drive.join("wamp64/bin/mysql"),
            system_drive.join("wamp64/bin/mariadb"),
        ];
        for root in known_roots {
            if let Some(binary) = find_mysql_binary_in_tree(&root, &executable_names, 2) {
                candidates.push(binary);
            }
            candidates.extend(
                executable_names
                    .iter()
                    .map(|name| root.join("bin").join(name)),
            );
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            format!(
                "MySQL/MariaDB client ({}) tidak ditemukan. Pastikan MySQL/MariaDB terpasang dan folder bin berisi executable tersebut ada di PATH. Di Windows, lokasi XAMPP, Laragon, WAMP, dan instalasi standar juga dicari; setelah mengubah PATH, buka ulang aplikasi.",
                executable_names.join(" atau ")
            )
        })
}

#[cfg(windows)]
fn find_mysql_binary_in_tree(
    directory: &Path,
    executable_names: &[String],
    depth: usize,
) -> Option<PathBuf> {
    for name in executable_names {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if depth == 0 {
        return None;
    }

    let entries = std::fs::read_dir(directory).ok()?;
    for entry in entries.flatten() {
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        if let Some(binary) = find_mysql_binary_in_tree(&entry.path(), executable_names, depth - 1)
        {
            return Some(binary);
        }
        let bin_directory = entry.path().join("bin");
        for name in executable_names {
            let candidate = bin_directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Sets the OS pipe buffer capacity (e.g. 256 KB – 512 KB) to reduce context-switching during high-speed data stream.
#[cfg(unix)]
pub fn tune_pipe_capacity<T: std::os::unix::io::AsRawFd>(target: &T, size_bytes: usize) {
    #[cfg(target_os = "linux")]
    {
        let fd = target.as_raw_fd();
        // F_SETPIPE_SZ is 1031 on Linux kernel 2.6.35+
        unsafe {
            libc::fcntl(fd, 1031, size_bytes as libc::c_int);
        }
    }
}
