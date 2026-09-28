use crate::{config, plan, runtime, scan};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, Write},
    os::unix::process::CommandExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn dap_log(msg: &str) {
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/java-launcher-dap.log")
    {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let _ = writeln!(f, "[{ms}] [PID:{}] {msg}", std::process::id());
    }
}

pub fn read_dap_message<R: BufRead>(reader: &mut R) -> Result<Option<Value>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(rest.trim().parse::<usize>()?);
        }
    }

    let length = match content_length {
        Some(len) => len,
        None => return Ok(None),
    };

    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    let val: Value = serde_json::from_slice(&body)?;
    Ok(Some(val))
}

pub fn write_dap_message<W: Write>(writer: &mut W, val: &Value) -> Result<()> {
    let body = serde_json::to_string(val)?;
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    writer.flush()?;
    Ok(())
}

pub struct DapServer<W: Write> {
    writer: Arc<Mutex<W>>,
    seq: AtomicU64,
    root: PathBuf,
    config_path: PathBuf,
    child: Arc<Mutex<Option<Child>>>,
    group_members: Arc<Mutex<Vec<String>>>,
    entry_id: Arc<Mutex<Option<String>>>,
    display_name: Arc<Mutex<Option<String>>>,
    terminated: Arc<AtomicBool>,
}

impl<W: Write + Send + 'static> DapServer<W> {
    pub fn new(writer: W, root: PathBuf, config_path: PathBuf) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
            seq: AtomicU64::new(1),
            root,
            config_path,
            child: Arc::new(Mutex::new(None)),
            group_members: Arc::new(Mutex::new(Vec::new())),
            entry_id: Arc::new(Mutex::new(None)),
            display_name: Arc::new(Mutex::new(None)),
            terminated: Arc::new(AtomicBool::new(false)),
        }
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst)
    }

    fn send(&self, val: &Value) -> Result<()> {
        dap_log(&format!("-> {}", val));
        let mut w = self.writer.lock().unwrap();
        write_dap_message(&mut *w, val)
    }

    fn send_response(
        &self,
        request_seq: u64,
        command: &str,
        success: bool,
        body: Option<Value>,
        message: Option<&str>,
    ) -> Result<()> {
        let seq = self.next_seq();
        let mut resp = json!({
            "seq": seq,
            "type": "response",
            "request_seq": request_seq,
            "command": command,
            "success": success,
        });
        if let Some(b) = body {
            resp["body"] = b;
        }
        if let Some(m) = message {
            resp["message"] = json!(m);
        }
        self.send(&resp)
    }

    fn send_event(&self, event: &str, body: Option<Value>) -> Result<()> {
        let seq = self.next_seq();
        let mut msg = json!({
            "seq": seq,
            "type": "event",
            "event": event,
        });
        if let Some(b) = body {
            msg["body"] = b;
        }
        self.send(&msg)
    }

    fn send_output(&self, category: &str, text: &str) -> Result<()> {
        self.send_event(
            "output",
            Some(json!({
                "category": category,
                "output": text,
            })),
        )
    }

    pub fn run<R: BufRead>(&self, mut reader: R) -> Result<()> {
        dap_log("DAP server session started");
        while let Some(msg) = read_dap_message(&mut reader)? {
            dap_log(&format!("<- {}", msg));
            let msg_type = msg.get("type").and_then(Value::as_str).unwrap_or("");
            if msg_type == "request" {
                let seq = msg.get("seq").and_then(Value::as_u64).unwrap_or(0);
                let command = msg.get("command").and_then(Value::as_str).unwrap_or("");
                let args = msg.get("arguments").cloned().unwrap_or(json!({}));
                if self.handle_request(seq, command, &args)? {
                    break;
                }
            } else if msg_type == "response" {
                dap_log(&format!("Received reverse request response: {:?}", msg));
            }
        }
        dap_log("DAP server session ended");
        Ok(())
    }

    fn handle_request(&self, req_seq: u64, command: &str, args: &Value) -> Result<bool> {
        match command {
            "initialize" => {
                let body = json!({
                    "supportsConfigurationDoneRequest": true,
                    "supportsTerminateRequest": true,
                    "supportsRestartRequest": true,
                });
                self.send_response(req_seq, command, true, Some(body), None)?;
                self.send_event("initialized", None)?;
            }
            "configurationDone" => {
                self.send_response(req_seq, command, true, None, None)?;
            }
            "setBreakpoints" => {
                let body = json!({
                    "breakpoints": []
                });
                self.send_response(req_seq, command, true, Some(body), None)?;
            }
            "setExceptionBreakpoints" => {
                self.send_response(req_seq, command, true, None, None)?;
            }
            "threads" => {
                let name = self
                    .display_name
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap_or_else(|| "main".to_string());
                let body = json!({
                    "threads": [
                        { "id": 1, "name": name }
                    ]
                });
                self.send_response(req_seq, command, true, Some(body), None)?;
            }
            "stackTrace" => {
                let body = json!({
                    "stackFrames": [],
                    "totalFrames": 0
                });
                self.send_response(req_seq, command, true, Some(body), None)?;
            }
            "scopes" => {
                let body = json!({
                    "scopes": []
                });
                self.send_response(req_seq, command, true, Some(body), None)?;
            }
            "variables" => {
                let body = json!({
                    "variables": []
                });
                self.send_response(req_seq, command, true, Some(body), None)?;
            }
            "disconnect" | "terminate" => {
                dap_log("Handling disconnect/terminate");
                self.terminated.store(true, Ordering::SeqCst);
                if let Some(mut child) = self.child.lock().unwrap().take() {
                    let pid = child.id() as i32;
                    unsafe {
                        libc::kill(-pid, libc::SIGTERM);
                    }
                    let _ = child.kill();
                    dap_log(&format!("Killed child process PID {pid}"));
                }
                if let Some(ref id) = *self.entry_id.lock().unwrap() {
                    if let Ok(dir) = runtime::state_dir(&self.root) {
                        let _ = std::fs::remove_file(dir.join(format!("{}.pid", runtime::hash(id))));
                    }
                }
                let members = self.group_members.lock().unwrap().clone();
                if !members.is_empty() {
                    if let Ok(dir) = runtime::state_dir(&self.root) {
                        for entry in members {
                            let pid_file = dir.join(format!("{}.pid", runtime::hash(&entry)));
                            if let Ok(content) = std::fs::read_to_string(&pid_file) {
                                if let Ok(pid) = content.trim().parse::<i32>() {
                                    unsafe {
                                        libc::kill(-pid, libc::SIGTERM);
                                    }
                                    dap_log(&format!("Killed group member {} (PID {})", entry, pid));
                                }
                            }
                            let _ = std::fs::remove_file(&pid_file);
                        }
                    }
                }
                self.send_response(req_seq, command, true, None, None)?;
                self.send_event("terminated", None)?;
                return Ok(true);
            }
            "launch" => {
                self.handle_launch(req_seq, args)?;
            }
            _ => {
                dap_log(&format!("Unhandled request: {command}"));
                self.send_response(req_seq, command, true, None, None)?;
            }
        }
        Ok(false)
    }

    fn handle_launch(&self, req_seq: u64, args: &Value) -> Result<()> {
        if let Some(group_name) = args.get("group").and_then(Value::as_str) {
            self.launch_group(req_seq, group_name)?;
        } else {
            self.launch_single(req_seq, args)?;
        }
        Ok(())
    }

    fn launch_group(&self, req_seq: u64, group_name: &str) -> Result<()> {
        dap_log(&format!("Launching group: {group_name}"));
        *self.display_name.lock().unwrap() = Some(format!("Group: {group_name}"));
        let mut config = config::load(&self.config_path).unwrap_or_default();
        if config.groups.is_empty() {
            let project = scan::scan(&self.root)?;
            let _ = config::import_vscode(&project, &mut config);
        }

        let items = match config.groups.get(group_name) {
            Some(items) => items.clone(),
            None => {
                let err_msg = format!("Unknown group '{group_name}'");
                dap_log(&err_msg);
                self.send_response(req_seq, "launch", false, None, Some(&err_msg))?;
                return Ok(());
            }
        };

        let enabled: Vec<_> = items.into_iter().filter(|i| i.enabled).collect();
        if enabled.is_empty() {
            let err_msg = format!("Group '{group_name}' has no enabled items");
            self.send_response(req_seq, "launch", false, None, Some(&err_msg))?;
            return Ok(());
        }

        let entries: Vec<String> = enabled.iter().map(|i| i.entry.clone()).collect();
        *self.group_members.lock().unwrap() = entries;

        self.send_output(
            "stdout",
            &format!(
                "[Java Launcher] 🚀 Orchestrating group '{group_name}' ({} services)...\n",
                enabled.len()
            ),
        )?;

        // Send DAP reverse request `startDebugging` for each enabled service
        for item in &enabled {
            let child_seq = self.next_seq();
            let label = item.name.clone().unwrap_or_else(|| {
                item.entry
                    .rsplit('.')
                    .next()
                    .unwrap_or(&item.entry)
                    .to_string()
            });
            let start_debug_req = json!({
                "seq": child_seq,
                "type": "request",
                "command": "startDebugging",
                "arguments": {
                    "request": "launch",
                    "configuration": {
                        "name": label,
                        "adapter": "java-launcher",
                        "request": "launch",
                        "label": label,
                        "entry": item.entry,
                        "skipBuild": true
                    }
                }
            });
            dap_log(&format!(
                "Sending startDebugging reverse request: {}",
                start_debug_req
            ));
            self.send(&start_debug_req)?;
            self.send_output(
                "stdout",
                &format!("[Java Launcher] ↳ Dispatched '{}'\n", label),
            )?;
            if item.delay_ms > 0 {
                thread::sleep(Duration::from_millis(item.delay_ms));
            }
        }

        self.send_response(req_seq, "launch", true, None, None)?;
        self.send_output(
            "stdout",
            &format!(
                "[Java Launcher] ✅ All {} services in group '{group_name}' dispatched to Zed.\n",
                enabled.len()
            ),
        )?;
        Ok(())
    }

    fn launch_single(&self, req_seq: u64, args: &Value) -> Result<()> {
        let entry_query = args
            .get("entry")
            .and_then(Value::as_str)
            .or_else(|| args.get("mainClass").and_then(Value::as_str))
            .or_else(|| args.get("label").and_then(Value::as_str))
            .context("Missing required 'entry' or 'mainClass' in launch configuration")?;

        dap_log(&format!("Launching single service: {entry_query}"));
        let project = scan::scan(&self.root)?;
        let entry = project.entry(entry_query)?;

        let display_name = args
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| args.get("label").and_then(Value::as_str))
            .map(ToString::to_string)
            .unwrap_or_else(|| {
                entry
                    .class
                    .rsplit('.')
                    .next()
                    .unwrap_or(&entry.id)
                    .to_string()
            });
        *self.display_name.lock().unwrap() = Some(display_name.clone());

        let config = config::load(&self.config_path).unwrap_or_default();
        let mut options = config.options(&entry.id);

        let skip_build = args
            .get("skipBuild")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if skip_build {
            options.build = Some(false);
        }

        let debug_port = args
            .get("debugPort")
            .and_then(Value::as_u64)
            .map(|p| p as u16);

        let mut plan = plan::build(&project, entry, &options, debug_port, false)?;
        let env = config::environment(&self.root, &options)?;

        // Ensure classpath file exists
        let need_classpath = plan
            .classpath_file
            .as_ref()
            .map(|f| !f.exists())
            .unwrap_or(false);

        // Execute prepare (compilation / classpath generation)
        for prep in &plan.prepare {
            let is_compile = prep
                .args
                .iter()
                .any(|a| a == "test-compile" || a == "compile");
            if is_compile && skip_build {
                continue;
            }
            if !is_compile && skip_build && !need_classpath {
                continue;
            }
            self.send_output(
                "stdout",
                &format!(
                    "[Java Launcher] Preparing {}: {} {}\n",
                    entry.id,
                    prep.program,
                    prep.args.join(" ")
                ),
            )?;
            let mut cmd = Command::new(&prep.program);
            cmd.args(&prep.args).current_dir(&prep.cwd);
            for (k, v) in &env {
                cmd.env(k, v);
            }
            let status = cmd
                .status()
                .with_context(|| format!("Failed to run build command: {}", prep.program))?;
            if !status.success() {
                let err_msg = format!("Preparation failed with exit code {:?}", status.code());
                self.send_response(req_seq, "launch", false, None, Some(&err_msg))?;
                return Ok(());
            }
        }

        plan::resolve_classpath(&mut plan)?;

        self.send_output(
            "stdout",
            &format!(
                "[Java Launcher] Starting {} ({})\n",
                entry.id, plan.launch.program
            ),
        )?;

        let mut cmd = Command::new(&plan.launch.program);
        cmd.args(&plan.launch.args);
        cmd.current_dir(&plan.launch.cwd);
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn {}", plan.launch.program))?;
        let pid = child.id();
        dap_log(&format!("Spawned child process PID {pid}"));

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        *self.child.lock().unwrap() = Some(child);
        *self.entry_id.lock().unwrap() = Some(entry.id.clone());
        if let Ok(dir) = runtime::state_dir(&self.root) {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(
                dir.join(format!("{}.pid", runtime::hash(&entry.id))),
                pid.to_string(),
            );
        }

        // DAP events
        self.send_event(
            "process",
            Some(json!({
                "name": display_name,
                "systemProcessId": pid,
                "isLocalProcess": true,
                "startMethod": "launch",
            })),
        )?;
        self.send_event(
            "thread",
            Some(json!({
                "reason": "started",
                "threadId": 1,
            })),
        )?;
        self.send_response(req_seq, "launch", true, None, None)?;

        // Pipe stdout
        if let Some(stdout) = stdout {
            let writer = self.writer.clone();
            let seq = self.seq.fetch_add(0, Ordering::SeqCst);
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().flatten() {
                    let mut w = writer.lock().unwrap();
                    let _ = write_dap_message(
                        &mut *w,
                        &json!({
                            "seq": seq,
                            "type": "event",
                            "event": "output",
                            "body": {
                                "category": "stdout",
                                "output": format!("{line}\n"),
                            }
                        }),
                    );
                }
            });
        }

        // Pipe stderr
        if let Some(stderr) = stderr {
            let writer = self.writer.clone();
            let seq = self.seq.fetch_add(0, Ordering::SeqCst);
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    let mut w = writer.lock().unwrap();
                    let _ = write_dap_message(
                        &mut *w,
                        &json!({
                            "seq": seq,
                            "type": "event",
                            "event": "output",
                            "body": {
                                "category": "stderr",
                                "output": format!("{line}\n"),
                            }
                        }),
                    );
                }
            });
        }

        // Wait thread
        let child_arc = self.child.clone();
        let writer = self.writer.clone();
        let terminated = self.terminated.clone();
        let entry_id_arc = self.entry_id.clone();
        let root = self.root.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(500));
                if terminated.load(Ordering::SeqCst) {
                    break;
                }
                let mut guard = child_arc.lock().unwrap();
                if let Some(ref mut child) = *guard {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            dap_log(&format!("Child process exited with status: {status}"));
                            if let Some(ref id) = *entry_id_arc.lock().unwrap() {
                                if let Ok(dir) = runtime::state_dir(&root) {
                                    let _ = std::fs::remove_file(dir.join(format!("{}.pid", runtime::hash(id))));
                                }
                            }
                            let mut w = writer.lock().unwrap();
                            let _ = write_dap_message(
                                &mut *w,
                                &json!({
                                    "seq": 0,
                                    "type": "event",
                                    "event": "output",
                                    "body": {
                                        "category": "stdout",
                                        "output": format!("[Java Launcher] Process exited with {}\n", status),
                                    }
                                }),
                            );
                            let _ = write_dap_message(
                                &mut *w,
                                &json!({
                                    "seq": 0,
                                    "type": "event",
                                    "event": "exited",
                                    "body": {
                                        "exitCode": status.code().unwrap_or(0),
                                    }
                                }),
                            );
                            let _ = write_dap_message(
                                &mut *w,
                                &json!({
                                    "seq": 0,
                                    "type": "event",
                                    "event": "terminated",
                                }),
                            );
                            *guard = None;
                            break;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            dap_log(&format!("Error waiting on child: {e}"));
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_dap_threads_return_service_and_group_name() {
        #[derive(Clone)]
        struct SharedWriter(Arc<Mutex<Vec<u8>>>);
        impl Write for SharedWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().write(buf)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.0.lock().unwrap().flush()
            }
        }

        let output = Arc::new(Mutex::new(Vec::new()));
        let writer = SharedWriter(output.clone());
        let server = DapServer::new(
            writer,
            PathBuf::from("/nonexistent"),
            PathBuf::from("/nonexistent"),
        );

        // Before launch: threads returns "main"
        server.handle_request(1, "threads", &json!({})).unwrap();
        let msgs = output.lock().unwrap().clone();
        let mut cursor = Cursor::new(msgs);
        let resp = read_dap_message(&mut cursor).unwrap().unwrap();
        assert_eq!(resp["body"]["threads"][0]["name"], "main");

        // Manually set display name as done during launch
        *server.display_name.lock().unwrap() = Some("SystemApplication".to_string());
        output.lock().unwrap().clear();
        server.handle_request(2, "threads", &json!({})).unwrap();
        let msgs = output.lock().unwrap().clone();
        let mut cursor = Cursor::new(msgs);
        let resp = read_dap_message(&mut cursor).unwrap().unwrap();
        assert_eq!(resp["body"]["threads"][0]["name"], "SystemApplication");

        // For group session
        *server.display_name.lock().unwrap() = Some("Group: uis".to_string());
        output.lock().unwrap().clear();
        server.handle_request(3, "threads", &json!({})).unwrap();
        let msgs = output.lock().unwrap().clone();
        let mut cursor = Cursor::new(msgs);
        let resp = read_dap_message(&mut cursor).unwrap().unwrap();
        assert_eq!(resp["body"]["threads"][0]["name"], "Group: uis");
    }
}
