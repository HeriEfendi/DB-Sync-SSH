use crate::models::{RemoteDbConfig, SshConfig};
use ssh2::Session;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub fn establish_ssh_session(config: &SshConfig) -> Result<Session, String> {
    let host = config.host.trim();
    if host.is_empty() {
        return Err("SSH Host tidak boleh kosong".to_string());
    }
    let port = if config.port == 0 { 22 } else { config.port };
    let addr_str = format!("{}:{}", host, port);

    let socket_addr = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("Gagal me-resolve host {}: {}", addr_str, e))?
        .next()
        .ok_or_else(|| format!("Host {} tidak dapat ditemukan", host))?;

    let tcp = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(10))
        .map_err(|e| format!("Gagal terhubung ke SSH {}: {}", addr_str, e))?;

    tcp.set_read_timeout(Some(Duration::from_secs(30))).ok();
    tcp.set_write_timeout(Some(Duration::from_secs(30))).ok();

    let mut session = Session::new().map_err(|e| format!("Gagal inisialisasi sesi SSH: {}", e))?;
    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(|e| format!("Handshake SSH gagal ke {}: {}", addr_str, e))?;

    let username = config.username.trim();
    if username.is_empty() {
        return Err("SSH Username tidak boleh kosong".to_string());
    }

    match config.auth_method.as_str() {
        "password" => {
            let pass = config.password.as_deref().unwrap_or("");
            session
                .userauth_password(username, pass)
                .map_err(|e| format!("Autentikasi SSH Password gagal: {}", e))?;
        }
        "key_no_pass" | "key_with_passphrase" => {
            let key_path = config
                .private_key_path
                .as_deref()
                .ok_or_else(|| "File private key path tidak ditentukan".to_string())?;

            let expanded_path = if key_path.starts_with("~/") {
                if let Some(home) = dirs::home_dir() {
                    home.join(&key_path[2..]).to_string_lossy().to_string()
                } else {
                    key_path.to_string()
                }
            } else {
                key_path.to_string()
            };

            let path = Path::new(&expanded_path);
            if !path.exists() {
                return Err(format!("File private key tidak ditemukan: {}", expanded_path));
            }

            let passphrase = if config.auth_method == "key_with_passphrase" {
                config.passphrase.as_deref()
            } else {
                None
            };

            session
                .userauth_pubkey_file(username, None, path, passphrase)
                .map_err(|e| format!("Autentikasi SSH Key gagal ({:?}): {}", path, e))?;
        }
        "agent" => {
            let mut agent = session
                .agent()
                .map_err(|e| format!("SSH Agent tidak tersedia: {}", e))?;
            agent
                .connect()
                .map_err(|e| format!("Gagal terhubung ke SSH Agent: {}", e))?;
            agent
                .list_identities()
                .map_err(|e| format!("Gagal membaca SSH Agent identities: {}", e))?;
            let identities = agent
                .identities()
                .map_err(|e| format!("Gagal memuat SSH Agent identities: {}", e))?;

            let mut authed = false;
            for identity in identities {
                if agent.userauth(username, &identity).is_ok() {
                    authed = true;
                    break;
                }
            }
            if !authed {
                return Err("Autentikasi SSH Agent gagal: Tidak ada key yang cocok".to_string());
            }
        }
        other => {
            return Err(format!("Metode autentikasi '{}' tidak dikenal", other));
        }
    }

    if !session.authenticated() {
        return Err("Autentikasi SSH gagal (sesi belum terotentikasi)".to_string());
    }

    Ok(session)
}

/// Execute a remote shell command via SSH channel and capture output
pub fn run_remote_ssh_command(session: &Session, cmd: &str) -> Result<(String, String, i32), String> {
    let mut channel = session
        .channel_session()
        .map_err(|e| format!("Gagal membuka SSH channel session: {}", e))?;

    channel
        .exec(cmd)
        .map_err(|e| format!("Gagal mengeksekusi perintah remote: {}", e))?;

    let mut stdout_buf = Vec::new();
    channel
        .read_to_end(&mut stdout_buf)
        .map_err(|e| format!("Gagal membaca stdout SSH: {}", e))?;

    let mut stderr_buf = Vec::new();
    channel
        .stderr()
        .read_to_end(&mut stderr_buf)
        .map_err(|e| format!("Gagal membaca stderr SSH: {}", e))?;

    channel.wait_close().ok();
    let exit_status = channel.exit_status().unwrap_or(0);

    let stdout_str = String::from_utf8_lossy(&stdout_buf).to_string();
    let stderr_str = String::from_utf8_lossy(&stderr_buf).to_string();

    Ok((stdout_str, stderr_str, exit_status))
}

pub struct SshTunnelHandle {
    pub local_port: u16,
    pub stop_flag: Arc<AtomicBool>,
}

impl Drop for SshTunnelHandle {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

/// Start an SSH port forwarding tunnel on a free local port
pub fn start_ssh_tunnel(
    ssh_config: &SshConfig,
    remote_db_config: &RemoteDbConfig,
) -> Result<SshTunnelHandle, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Gagal membuka port lokal untuk SSH tunnel: {}", e))?;
    let local_port = listener
        .local_addr()
        .map_err(|e| format!("Gagal mendapatkan local port: {}", e))?
        .port();

    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Gagal set nonblocking listener: {}", e))?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_thread = stop_flag.clone();
    let ssh_cfg = ssh_config.clone();
    let remote_host = if remote_db_config.host.trim().is_empty() {
        "127.0.0.1".to_string()
    } else {
        remote_db_config.host.trim().to_string()
    };
    let remote_port = if remote_db_config.port == 0 {
        3306
    } else {
        remote_db_config.port
    };

    std::thread::spawn(move || {
        while !stop_flag_thread.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut local_stream, _)) => {
                    let ssh_cfg_worker = ssh_cfg.clone();
                    let r_host = remote_host.clone();
                    let stop_worker = stop_flag_thread.clone();

                    std::thread::spawn(move || {
                        let session = match establish_ssh_session(&ssh_cfg_worker) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("[SshTunnel] Gagal membuat sesi SSH untuk client: {}", e);
                                return;
                            }
                        };
                        session.set_blocking(false);

                        let mut channel = loop {
                            if stop_worker.load(Ordering::SeqCst) {
                                return;
                            }
                            match session.channel_direct_tcpip(&r_host, remote_port, None) {
                                Ok(c) => break c,
                                Err(e) if e.code() == ssh2::ErrorCode::Session(-37) => {
                                    std::thread::sleep(Duration::from_millis(10));
                                }
                                Err(e) => {
                                    eprintln!("[SshTunnel] Gagal membuka direct-tcpip channel: {}", e);
                                    return;
                                }
                            }
                        };

                        local_stream.set_nonblocking(true).ok();
                        let mut local_buf = [0u8; 16384];
                        let mut remote_buf = [0u8; 16384];

                        while !stop_worker.load(Ordering::SeqCst) {
                            let mut did_work = false;

                            // 1. local -> remote
                            match local_stream.read(&mut local_buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    let mut written = 0;
                                    while written < n {
                                        match channel.write(&local_buf[written..n]) {
                                            Ok(w) => {
                                                written += w;
                                                did_work = true;
                                            }
                                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                                std::thread::sleep(Duration::from_millis(1));
                                            }
                                            Err(_) => return,
                                        }
                                    }
                                    let _ = channel.flush();
                                }
                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                                Err(_) => break,
                            }

                            // 2. remote -> local
                            match channel.read(&mut remote_buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    let mut written = 0;
                                    while written < n {
                                        match local_stream.write(&remote_buf[written..n]) {
                                            Ok(w) => {
                                                written += w;
                                                did_work = true;
                                            }
                                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                                std::thread::sleep(Duration::from_millis(1));
                                            }
                                            Err(_) => return,
                                        }
                                    }
                                    let _ = local_stream.flush();
                                }
                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                                Err(_) => break,
                            }

                            if !did_work {
                                std::thread::sleep(Duration::from_millis(1));
                            }
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    eprintln!("[SshTunnel] Error accept: {}", e);
                    break;
                }
            }
        }
    });

    Ok(SshTunnelHandle {
        local_port,
        stop_flag,
    })
}
