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
        // 1. Kumpulkan semua drive root yang terpasang (SystemDrive + C:\ s/d Z:\)
        let mut drives = Vec::new();
        if let Some(sys_drive) = std::env::var_os("SystemDrive") {
            let drive_str = format!("{}\\", sys_drive.to_string_lossy().trim_end_matches('\\'));
            drives.push(PathBuf::from(drive_str));
        } else {
            drives.push(PathBuf::from("C:\\"));
        }

        for letter in b'C'..=b'Z' {
            let drive_path = PathBuf::from(format!("{}:\\", letter as char));
            if drive_path.is_dir() && !drives.contains(&drive_path) {
                drives.push(drive_path);
            }
        }

        // 2. Kumpulkan folder Program Files (resmi MariaDB / MySQL installer)
        let mut program_dirs = Vec::new();
        for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
            if let Some(val) = std::env::var_os(var) {
                let p = PathBuf::from(val);
                if p.is_dir() && !program_dirs.contains(&p) {
                    program_dirs.push(p);
                }
            }
        }
        for drive in &drives {
            for sub in ["Program Files", "Program Files (x86)"] {
                let p = drive.join(sub);
                if p.is_dir() && !program_dirs.contains(&p) {
                    program_dirs.push(p);
                }
            }
        }

        // MSI MariaDB memasang ke folder berversi seperti "MariaDB 10.4", "MariaDB 10.11", "MariaDB 11.4"
        // MSI MySQL memasang ke "MySQL/MySQL Server 8.0" atau "MySQL Server 8.0"
        for p_dir in &program_dirs {
            if let Ok(entries) = std::fs::read_dir(p_dir) {
                for entry in entries.flatten() {
                    let is_dir = entry.file_type().map(|k| k.is_dir()).unwrap_or(false);
                    if !is_dir {
                        continue;
                    }
                    let folder_name = entry.file_name();
                    let name_lower = folder_name.to_string_lossy().to_ascii_lowercase();
                    if name_lower.starts_with("mariadb") || name_lower.starts_with("mysql") {
                        let bin_dir = entry.path().join("bin");
                        for name in &executable_names {
                            candidates.push(bin_dir.join(name));
                        }
                        if let Some(binary) = find_mysql_binary_in_tree(&entry.path(), &executable_names, 2) {
                            candidates.push(binary);
                        }
                    }
                }
            }
        }

        // 3. Lokasi stack umum (XAMPP, Laragon, WAMP, Standalone) di semua drive
        for drive in &drives {
            let direct_bins = [
                drive.join("xampp/mysql/bin"),
                drive.join("mariadb/bin"),
                drive.join("mysql/bin"),
            ];
            for bin_path in direct_bins {
                for name in &executable_names {
                    candidates.push(bin_path.join(name));
                }
            }

            let search_roots = [
                drive.join("laragon/bin/mysql"),
                drive.join("laragon/bin/mariadb"),
                drive.join("wamp64/bin/mysql"),
                drive.join("wamp64/bin/mariadb"),
                drive.join("wamp/bin/mysql"),
                drive.join("wamp/bin/mariadb"),
                drive.join("xampp/mysql"),
            ];
            for root in search_roots {
                if root.is_dir() {
                    if let Some(binary) = find_mysql_binary_in_tree(&root, &executable_names, 2) {
                        candidates.push(binary);
                    }
                    for name in &executable_names {
                        candidates.push(root.join("bin").join(name));
                    }
                }
            }
        }

        // 4. Package manager user profile (Scoop, Chocolatey, dsb)
        if let Some(user_profile) = std::env::var_os("USERPROFILE") {
            let up = PathBuf::from(user_profile);
            let scoop_bins = [
                up.join("scoop/apps/mariadb/current/bin"),
                up.join("scoop/apps/mysql/current/bin"),
                up.join("scoop/shims"),
            ];
            for sb in scoop_bins {
                for name in &executable_names {
                    candidates.push(sb.join(name));
                }
            }
        }

        let choco_bins = [
            PathBuf::from("C:\\ProgramData\\chocolatey\\bin"),
            PathBuf::from("C:\\tools\\mysql\\bin"),
            PathBuf::from("C:\\tools\\mariadb\\bin"),
        ];
        for cb in choco_bins {
            for name in &executable_names {
                candidates.push(cb.join(name));
            }
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            format!(
                "MySQL/MariaDB client ({}) tidak ditemukan. Pastikan MySQL/MariaDB terpasang dan folder bin berisi executable tersebut ada di PATH. Di Windows, lokasi resmi MariaDB/MySQL (Program Files), XAMPP, Laragon, WAMP di semua drive juga dicari otomatis; setelah mengubah PATH atau memasang client, buka ulang aplikasi.",
                executable_names.join(" atau ")
            )
        })
}

pub fn find_mysql_binary_in_tree(
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
        let bin_directory = entry.path().join("bin");
        for name in executable_names {
            let candidate = bin_directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        if let Some(binary) = find_mysql_binary_in_tree(&entry.path(), executable_names, depth - 1)
        {
            return Some(binary);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_find_mysql_binary_in_tree_mariadb_versioned_folder() {
        let unique = format!("db_sync_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let temp_dir = std::env::temp_dir().join(unique);
        let bin_dir = temp_dir.join("Program Files").join("MariaDB 10.11").join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let dummy_exe = bin_dir.join("mariadb");
        fs::write(&dummy_exe, b"dummy").unwrap();

        let found = find_mysql_binary_in_tree(
            &temp_dir.join("Program Files").join("MariaDB 10.11"),
            &["mariadb".to_string()],
            2,
        );

        assert_eq!(found, Some(dummy_exe));
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_find_mysql_binary_in_tree_laragon_nested_folder() {
        let unique = format!("db_sync_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let temp_dir = std::env::temp_dir().join(unique);
        let bin_dir = temp_dir
            .join("laragon")
            .join("bin")
            .join("mysql")
            .join("mysql-8.0.30-winx64")
            .join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let dummy_exe = bin_dir.join("mysql");
        fs::write(&dummy_exe, b"dummy").unwrap();

        let found = find_mysql_binary_in_tree(
            &temp_dir.join("laragon").join("bin").join("mysql"),
            &["mysql".to_string()],
            2,
        );

        assert_eq!(found, Some(dummy_exe));
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
