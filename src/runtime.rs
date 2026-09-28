//! Unix supervisor: only the owner of a live Child signals its process group.
//! Clients use a private local socket, never a guessed PID or a class-name search.
use crate::{
    config,
    model::{Entry, LaunchOptions, Project},
    plan::{self, Plan},
};
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))[..20].to_owned()
}

pub fn find_zed_ancestor() -> Option<u32> {
    if let Ok(val) = std::env::var("ZED_PID") {
        if let Ok(pid) = val.parse::<u32>() {
            return Some(pid);
        }
    }
    #[cfg(target_os = "macos")]
    {
        let mut current_pid = unsafe { libc::getppid() };
        for _ in 0..20 {
            if current_pid <= 1 {
                break;
            }
            let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
            let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
            let ret = unsafe {
                libc::proc_pidinfo(
                    current_pid,
                    libc::PROC_PIDTBSDINFO,
                    0,
                    &mut info as *mut _ as *mut libc::c_void,
                    size,
                )
            };
            if ret != size {
                break;
            }
            let name = unsafe {
                std::ffi::CStr::from_ptr(info.pbi_name.as_ptr())
                    .to_string_lossy()
                    .to_lowercase()
            };
            if name == "zed" || name == "zed-editor" || name.starts_with("zed") {
                return Some(current_pid as u32);
            }
            if info.pbi_ppid <= 1 || info.pbi_ppid as i32 == current_pid {
                break;
            }
            current_pid = info.pbi_ppid as i32;
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut current_pid = unsafe { libc::getppid() };
        for _ in 0..20 {
            if current_pid <= 1 {
                break;
            }
            if let Ok(comm) = std::fs::read_to_string(format!("/proc/{current_pid}/comm")) {
                let name = comm.trim().to_lowercase();
                if name == "zed" || name == "zed-editor" || name.starts_with("zed") {
                    return Some(current_pid as u32);
                }
            }
            if let Ok(stat) = std::fs::read_to_string(format!("/proc/{current_pid}/stat")) {
                if let Some(ppid_str) = stat.split_whitespace().nth(3) {
                    if let Ok(ppid) = ppid_str.parse::<i32>() {
                        if ppid <= 1 || ppid == current_pid {
                            break;
                        }
                        current_pid = ppid;
                        continue;
                    }
                }
            }
            break;
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn watch_process_exit(target_pid: u32, cancel: Arc<AtomicBool>) {
    thread::spawn(move || {
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            while unsafe { libc::kill(target_pid as i32, 0) == 0 } {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                thread::sleep(Duration::from_millis(500));
            }
            cancel.store(true, Ordering::Relaxed);
            return;
        }

        let ke = libc::kevent {
            ident: target_pid as usize,
            filter: libc::EVFILT_PROC,
            flags: libc::EV_ADD | libc::EV_ENABLE | libc::EV_ONESHOT,
            fflags: libc::NOTE_EXIT,
            data: 0,
            udata: std::ptr::null_mut(),
        };

        let ret = unsafe {
            libc::kevent(
                kq,
                &ke,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };

        if ret < 0 {
            unsafe { libc::close(kq) };
            if unsafe { libc::kill(target_pid as i32, 0) != 0 } {
                cancel.store(true, Ordering::Relaxed);
            }
            return;
        }

        let timeout = libc::timespec {
            tv_sec: 1,
            tv_nsec: 0,
        };

        let mut out_event: libc::kevent = unsafe { std::mem::zeroed() };
        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let n = unsafe {
                libc::kevent(
                    kq,
                    std::ptr::null(),
                    0,
                    &mut out_event,
                    1,
                    &timeout,
                )
            };
            if n > 0 {
                cancel.store(true, Ordering::Relaxed);
                break;
            } else if n < 0 {
                if unsafe { libc::kill(target_pid as i32, 0) != 0 } {
                    cancel.store(true, Ordering::Relaxed);
                }
                break;
            }
        }
        unsafe { libc::close(kq) };
    });
}

#[cfg(not(target_os = "macos"))]
fn watch_process_exit(target_pid: u32, cancel: Arc<AtomicBool>) {
    thread::spawn(move || {
        while unsafe { libc::kill(target_pid as i32, 0) == 0 } {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(Duration::from_millis(500));
        }
        cancel.store(true, Ordering::Relaxed);
    });
}
pub fn state_dir(root: &Path) -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is required")?;
    let base = std::env::var_os("JAVA_LAUNCHER_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(home).join(".local/state/java-launcher"));
    Ok(base.join(hash(&root.to_string_lossy())))
}
fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        bail!("Not a real private directory: {}", path.display());
    }
    use std::os::unix::fs::MetadataExt;
    if meta.uid() != unsafe { libc::geteuid() } {
        bail!("Directory is not owned by current user: {}", path.display());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
fn socket_path(root: &Path, id: &str) -> PathBuf {
    // Short path avoids macOS sockaddr_un's 104-byte limit. Parent is mode 0700.
    PathBuf::from(format!("/tmp/java-launcher-{}", unsafe { libc::geteuid() })).join(format!(
        "{}.sock",
        hash(&format!("{}::{id}", root.display()))
    ))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub entry: String,
    pub phase: String,
    pub supervisor_pid: u32,
    pub child_pid: Option<u32>,
    pub started_at: u64,
    pub debug_port: Option<u16>,
    pub log: String,
    #[serde(default)]
    pub watch_pid: Option<u32>,
}
pub fn request(root: &Path, id: &str, operation: &str) -> Result<Status> {
    let mut stream =
        UnixStream::connect(socket_path(root, id)).context("No active managed process")?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(operation.as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut response = String::new();
    stream.take(16 * 1024).read_to_string(&mut response)?;
    serde_json::from_str(&response).context("Invalid supervisor response")
}
pub fn statuses(root: &Path) -> Result<Vec<Status>> {
    let dir = state_dir(root)?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut statuses = vec![];
    for f in fs::read_dir(dir)? {
        let p = f?.path();
        if p.extension().is_some_and(|e| e == "json") {
            if let Ok(old) = config::read_json::<Status>(&p) {
                if let Ok(current) = request(root, &old.entry, "status") {
                    statuses.push(current);
                }
            }
        }
    }
    statuses.sort_by(|a, b| a.entry.cmp(&b.entry));
    Ok(statuses)
}
pub fn stop(root: &Path, id: &str) -> Result<()> {
    let status = request(root, id, "stop")?;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        match request(root, id, "status") {
            Err(_) => return Ok(()),
            Ok(s) if s.supervisor_pid != status.supervisor_pid => {
                bail!("A new instance appeared while stopping {id}; not stopping it")
            }
            _ => thread::sleep(Duration::from_millis(100)),
        }
    }
    bail!("Timed out stopping {id}; inspect the supervisor/log. No unrelated PID was killed.")
}

struct Supervisor {
    root: PathBuf,
    listener: UnixListener,
    socket: PathBuf,
    status_file: PathBuf,
    status: Status,
    cancel: Arc<AtomicBool>,
    _lock: File,
    signals: Vec<signal_hook::SigId>,
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        for id in &self.signals {
            signal_hook::low_level::unregister(*id);
        }
        let _ = fs::remove_file(&self.socket);
        let _ = fs::remove_file(&self.status_file);
    }
}
impl Supervisor {
    fn new(root: &Path, id: &str, debug_port: Option<u16>, watch_pid: Option<u32>) -> Result<Self> {
        let dir = state_dir(root)?;
        private_dir(&dir)?;
        let key = hash(id);
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(dir.join(format!("{key}.lock")))?;
        FileExt::try_lock_exclusive(&lock)
            .context("Entry is already running (or being prepared)")?;
        let socket = socket_path(root, id);
        private_dir(socket.parent().unwrap())?;
        if socket.exists() {
            fs::remove_file(&socket)?;
        }
        let listener = UnixListener::bind(&socket)?;
        listener.set_nonblocking(true)?;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(pid) = watch_pid {
            watch_process_exit(pid, Arc::clone(&cancel));
        }
        let mut signals = vec![];
        for signal in [
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGHUP,
        ] {
            signals.push(signal_hook::flag::register(signal, Arc::clone(&cancel))?);
        }
        let status = Status {
            entry: id.into(),
            phase: "preparing".into(),
            supervisor_pid: std::process::id(),
            child_pid: None,
            started_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            debug_port,
            log: dir
                .join(format!("{key}.log"))
                .to_string_lossy()
                .into_owned(),
            watch_pid,
        };
        let status_file = dir.join(format!("{key}.json"));
        config::atomic_json(&status_file, &status)?;
        Ok(Self {
            root: root.into(),
            listener,
            socket,
            status_file,
            status,
            cancel,
            _lock: lock,
            signals,
        })
    }
    fn poll(&mut self) -> Result<()> {
        loop {
            match self.listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_read_timeout(Some(Duration::from_millis(200)))?;
                    stream.set_write_timeout(Some(Duration::from_millis(200)))?;
                    let mut op = String::new();
                    if (&mut stream).take(32).read_to_string(&mut op).is_ok() {
                        if op == "stop" {
                            self.cancel.store(true, Ordering::Relaxed);
                        }
                        let _ = serde_json::to_writer(&mut stream, &self.status);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
    fn phase(&mut self, phase: &str, pid: Option<u32>) -> Result<()> {
        self.status.phase = phase.into();
        self.status.child_pid = pid;
        config::atomic_json(&self.status_file, &self.status)?;
        Ok(())
    }
    fn build_lock(&mut self) -> Result<File> {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(state_dir(&self.root)?.join("build.lock"))?;
        self.phase("waiting_for_build", None)?;
        loop {
            match FileExt::try_lock_exclusive(&lock) {
                Ok(()) => return Ok(lock),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e.into()),
            }
            self.poll()?;
            if self.cancel.load(Ordering::Relaxed) {
                bail!("Cancelled before build");
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
    fn execute(
        &mut self,
        spec: &plan::CommandSpec,
        env: &BTreeMap<String, String>,
        phase: &str,
    ) -> Result<()> {
        self.poll()?;
        if self.cancel.load(Ordering::Relaxed) {
            bail!("Cancelled");
        }
        eprintln!("[java-launcher] {}: {phase}", self.status.entry);
        let mut command = plan::command(spec, env);
        command.process_group(0);
        let child = command
            .spawn()
            .with_context(|| format!("Could not start {}", spec.program))?;
        let mut owned = OwnedChild {
            child,
            finished: false,
        };
        self.phase(phase, Some(owned.child.id()))?;
        loop {
            self.poll()?;
            if self.cancel.load(Ordering::Relaxed) {
                self.phase("stopping", Some(owned.child.id()))?;
                owned.terminate()?;
                bail!("Cancelled");
            }
            if let Some(exit) = owned.child.try_wait()? {
                owned.finished = true;
                self.phase("between_steps", None)?;
                if !exit.success() {
                    bail!("{} exited with {exit}", spec.program);
                }
                return Ok(());
            }
            thread::sleep(Duration::from_millis(40));
        }
    }
}
struct OwnedChild {
    child: Child,
    finished: bool,
}
impl OwnedChild {
    fn terminate(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        // Child has not been reaped: PID cannot be reused here. All group signals
        // are sent before wait()/try_wait() can reap it.
        let pgid = self.child.id() as i32;
        unsafe {
            libc::kill(-pgid, libc::SIGTERM);
        }
        // Keep leader unreaped through the grace period to avoid PID reuse.
        thread::sleep(Duration::from_millis(800));
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
        self.child.wait()?;
        self.finished = true;
        Ok(())
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

pub fn run(
    project: &Project,
    entry: &Entry,
    options: &LaunchOptions,
    mut plan: Plan,
    debug_port: Option<u16>,
    prepare_only: bool,
    watch_pid: Option<u32>,
) -> Result<()> {
    let env = config::environment(&project.root, options)?;
    let mut supervisor = Supervisor::new(&project.root, &entry.id, debug_port, watch_pid)?;
    let build_lock = supervisor.build_lock()?;
    if let Some(cp) = &plan.classpath_file {
        if cp.exists() {
            fs::remove_file(cp)?;
        }
    }
    for classes in &plan.classes {
        fs::create_dir_all(classes)?;
    }
    for step in &plan.prepare {
        supervisor.execute(step, &env, "building")?;
    }
    plan::resolve_classpath(&mut plan)?;
    drop(build_lock);
    if !prepare_only {
        supervisor.execute(&plan.launch, &env, "running")?;
    }
    Ok(())
}

pub fn start(
    root: &Path,
    config_path: &Path,
    id: &str,
    skip_build: bool,
    debug_port: Option<u16>,
    suspend: bool,
    watch_pid: Option<u32>,
) -> Result<Status> {
    if request(root, id, "status").is_ok() {
        bail!("Already running: {id}");
    }
    let dir = state_dir(root)?;
    private_dir(&dir)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(dir.join(format!("{}.log", hash(id))))?;
    let mut cmd = std::process::Command::new(std::env::current_exe()?);
    cmd.arg("--project")
        .arg(root)
        .arg("--config")
        .arg(config_path)
        .arg("run")
        .arg(id);
    if skip_build {
        cmd.arg("--skip-build");
    }
    if let Some(port) = debug_port {
        cmd.args(["--debug-port", &port.to_string()]);
    }
    if suspend {
        cmd.arg("--suspend");
    }
    let resolved_watch_pid = watch_pid.or_else(|| {
        let options = config::load(config_path).ok().map(|c| c.options(id))?;
        if options.terminate_on_zed_quit.unwrap_or(true) {
            find_zed_ancestor()
        } else {
            None
        }
    });
    if let Some(pid) = resolved_watch_pid {
        cmd.args(["--watch-pid", &pid.to_string()]);
    }
    cmd.stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .process_group(0);
    let mut child = cmd.spawn()?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Some(exit) = child.try_wait()? {
            bail!(
                "Supervisor exited with {exit}; see {}",
                dir.join(format!("{}.log", hash(id))).display()
            );
        }
        if let Ok(status) = request(root, id, "status") {
            if status.supervisor_pid != child.id() {
                bail!("Another instance won the start race: {id}");
            }
            // Detach after acknowledgement. 'building' is not application readiness.
            thread::spawn(move || {
                let _ = child.wait();
            });
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
    bail!("Supervisor did not acknowledge startup within 30 seconds")
}

pub fn open_in_zed(path: &Path) -> Result<()> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path);
    }

    if std::process::Command::new("zed")
        .arg(path)
        .spawn()
        .is_ok()
    {
        return Ok(());
    }

    if std::process::Command::new("/usr/local/bin/zed")
        .arg(path)
        .spawn()
        .is_ok()
    {
        return Ok(());
    }

    if std::process::Command::new("open")
        .args(["-a", "Zed"])
        .arg(path)
        .spawn()
        .is_ok()
    {
        return Ok(());
    }

    bail!("Failed to launch Zed editor. Make sure 'zed' CLI or Zed.app is installed.")
}
