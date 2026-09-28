use crate::{config, plan, runtime, scan};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
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

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    if s.len() % 2 != 0 {
        return Err(());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

fn find_jdtls_proxy_port(root: &Path) -> Option<u16> {
    let hex_root: String = root
        .to_string_lossy()
        .as_bytes()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    let home = dirs_home()?;
    let candidate_dirs = [
        home.join("Library/Application Support/Zed/extensions/work/java/proxy"),
        home.join("Library/Application Support/Zed (Preview)/extensions/work/java/proxy"),
        home.join(".local/share/zed/extensions/work/java/proxy"),
        home.join(".local/share/zed-preview/extensions/work/java/proxy"),
    ];

    for dir in &candidate_dirs {
        let file = dir.join(&hex_root);
        if file.exists() {
            if let Ok(content) = std::fs::read_to_string(&file) {
                if let Ok(port) = content.trim().parse::<u16>() {
                    dap_log(&format!("Found JDTLS proxy port {port} at {}", file.display()));
                    return Some(port);
                }
            }
        }
    }

    for dir in &candidate_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Ok(decoded_bytes) = hex_decode(&name) {
                    if let Ok(decoded_path) = String::from_utf8(decoded_bytes) {
                        if root.starts_with(&decoded_path) || Path::new(&decoded_path).starts_with(root) {
                            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                                if let Ok(port) = content.trim().parse::<u16>() {
                                    dap_log(&format!("Found matching JDTLS proxy port {port} for {}", decoded_path));
                                    return Some(port);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn request_jdtls_dap_port(http_port: u16) -> Result<u16> {
    let mut stream = TcpStream::connect(("127.0.0.1", http_port))
        .with_context(|| format!("Failed to connect to JDTLS HTTP proxy on port {http_port}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let payload = serde_json::to_string(&json!({
        "method": "workspace/executeCommand",
        "params": {
            "command": "vscode.java.startDebugSession"
        }
    }))?;
    let req = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1:{http_port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(req.as_bytes())?;
    stream.flush()?;

    let mut resp = String::new();
    stream.read_to_string(&mut resp)?;

    let body_start = resp
        .find("\r\n\r\n")
        .context("Invalid HTTP response from JDTLS proxy")?
        + 4;
    let body = &resp[body_start..];
    let parsed: Value =
        serde_json::from_str(body).context("Invalid JSON in JDTLS proxy response")?;
    let dap_port = parsed
        .get("result")
        .and_then(Value::as_u64)
        .context("No DAP port returned by JDTLS")? as u16;

    dap_log(&format!("Allocated JDTLS DAP port {dap_port}"));
    Ok(dap_port)
}

fn allocate_free_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

pub struct DapServer<W: Write> {
    writer: Arc<Mutex<W>>,
    seq: Arc<AtomicU64>,
    root: PathBuf,
    config_path: PathBuf,
    child: Arc<Mutex<Option<Child>>>,
    group_members: Arc<Mutex<Vec<String>>>,
    entry_id: Arc<Mutex<Option<String>>>,
    display_name: Arc<Mutex<Option<String>>>,
    terminated: Arc<AtomicBool>,
    is_stopped: Arc<AtomicBool>,
    initialized_sent: Arc<AtomicBool>,
    jdtls_writer: Arc<Mutex<Option<TcpStream>>>,
    jdtls_reader: Arc<Mutex<Option<BufReader<TcpStream>>>>,
    jdtls_seq: Arc<AtomicU64>,
    pending_breakpoints: Arc<Mutex<Vec<Value>>>,
    last_stopped_thread_id: Arc<AtomicI64>,
}

impl<W: Write + Send + 'static> DapServer<W> {
    pub fn new(writer: W, root: PathBuf, config_path: PathBuf, initial_name: Option<String>) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
            seq: Arc::new(AtomicU64::new(1)),
            root,
            config_path,
            child: Arc::new(Mutex::new(None)),
            group_members: Arc::new(Mutex::new(Vec::new())),
            entry_id: Arc::new(Mutex::new(None)),
            display_name: Arc::new(Mutex::new(initial_name)),
            terminated: Arc::new(AtomicBool::new(false)),
            is_stopped: Arc::new(AtomicBool::new(false)),
            initialized_sent: Arc::new(AtomicBool::new(false)),
            jdtls_writer: Arc::new(Mutex::new(None)),
            jdtls_reader: Arc::new(Mutex::new(None)),
            jdtls_seq: Arc::new(AtomicU64::new(1000)),
            pending_breakpoints: Arc::new(Mutex::new(Vec::new())),
            last_stopped_thread_id: Arc::new(AtomicI64::new(0)),
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
                if self.handle_request(seq, command, &args, &msg)? {
                    break;
                }
            } else if msg_type == "response" {
                dap_log(&format!("Received reverse request response: {:?}", msg));
            }
        }
        dap_log("DAP server session ended");
        Ok(())
    }

    fn handle_request(
        &self,
        req_seq: u64,
        command: &str,
        args: &Value,
        raw_msg: &Value,
    ) -> Result<bool> {
        match command {
            "initialize" => {
                let mut connected_jdtls = false;
                if let Some(proxy_port) = find_jdtls_proxy_port(&self.root) {
                    match request_jdtls_dap_port(proxy_port) {
                        Ok(dap_port) => match TcpStream::connect(("127.0.0.1", dap_port)) {
                            Ok(stream) => match stream.try_clone() {
                                Ok(reader_stream) => {
                                    let mut jdtls_w = stream;
                                    let mut jdtls_r = BufReader::new(reader_stream);
                                    let mut init_forward = raw_msg.clone();
                                    if let Some(args_obj) = init_forward
                                        .get_mut("arguments")
                                        .and_then(Value::as_object_mut)
                                    {
                                        args_obj.insert("adapterID".to_string(), json!("java"));
                                        args_obj.entry("linesStartAt1".to_string()).or_insert(json!(true));
                                        args_obj.entry("columnsStartAt1".to_string()).or_insert(json!(true));
                                        args_obj.entry("pathFormat".to_string()).or_insert(json!("path"));
                                    }
                                    if let Ok(()) =
                                        write_dap_message(&mut jdtls_w, &init_forward)
                                    {
                                        if let Ok(Some(mut resp)) =
                                            read_dap_message(&mut jdtls_r)
                                        {
                                            resp["request_seq"] = json!(req_seq);
                                            resp["seq"] = json!(self.next_seq());
                                            if let Some(body) = resp.get_mut("body").and_then(Value::as_object_mut) {
                                                body.insert("supportsTerminateRequest".to_string(), json!(true));
                                                body.insert("supportsTerminateThreadsRequest".to_string(), json!(true));
                                            }
                                            let _ = self.send(&resp);
                                            *self.jdtls_writer.lock().unwrap() =
                                                Some(jdtls_w);
                                            *self.jdtls_reader.lock().unwrap() =
                                                Some(jdtls_r);
                                            connected_jdtls = true;
                                            dap_log("Successfully initialized JDTLS DAP bridge");
                                        }
                                    }
                                }
                                Err(e) => {
                                    dap_log(&format!("Failed to clone JDTLS socket: {e}"));
                                }
                            },
                            Err(e) => {
                                dap_log(&format!("Failed to connect to JDTLS DAP port: {e}"));
                            }
                        },
                        Err(e) => {
                            dap_log(&format!("Failed to request JDTLS DAP port: {e}"));
                        }
                    }
                }

                if !connected_jdtls {
                    dap_log("Using standalone DAP initialization fallback");
                    let body = json!({
                        "supportsConfigurationDoneRequest": true,
                        "supportsTerminateRequest": true,
                        "supportsTerminateThreadsRequest": true,
                        "supportsRestartRequest": true,
                    });
                    self.send_response(req_seq, command, true, Some(body), None)?;
                    if !self.initialized_sent.swap(true, Ordering::SeqCst) {
                        self.send_event("initialized", None)?;
                    }
                }
            }
            "launch" => {
                self.handle_launch(req_seq, args)?;
            }
            "threads" => {
                let has_jdtls = self.jdtls_writer.lock().unwrap().is_some();
                let is_stopped = self.is_stopped.load(Ordering::SeqCst);
                if has_jdtls && is_stopped {
                    if let Some(ref mut jw) = *self.jdtls_writer.lock().unwrap() {
                        let _ = write_dap_message(jw, raw_msg);
                    }
                } else {
                    let name = self
                        .display_name
                        .lock()
                        .unwrap()
                        .clone()
                        .unwrap_or_else(|| "main".to_string());
                    let tid = self.last_stopped_thread_id.load(Ordering::SeqCst);
                    let body = json!({
                        "threads": [
                            { "id": if tid > 0 { tid } else { 1 }, "name": name }
                        ]
                    });
                    self.send_response(req_seq, command, true, Some(body), None)?;
                }
            }
            "disconnect" | "terminate" | "terminateThreads" => {
                dap_log(&format!("Handling {command}"));
                self.terminated.store(true, Ordering::SeqCst);
                if let Some(mut jw) = self.jdtls_writer.lock().unwrap().take() {
                    let jdtls_msg = if command == "terminateThreads" {
                        json!({
                            "seq": self.jdtls_seq.fetch_add(1, Ordering::SeqCst),
                            "type": "request",
                            "command": "disconnect",
                            "arguments": { "restart": false }
                        })
                    } else {
                        raw_msg.clone()
                    };
                    let _ = write_dap_message(&mut jw, &jdtls_msg);
                }
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
                self.send_event("exited", Some(json!({ "exitCode": 0 })))?;
                self.send_event("terminated", None)?;
                return Ok(true);
            }
            "setBreakpoints" => {
                let has_jdtls = self.jdtls_writer.lock().unwrap().is_some();
                let has_child = self.child.lock().unwrap().is_some();
                if has_jdtls && !has_child {
                    // Buffer breakpoints until JVM is attached
                    self.pending_breakpoints.lock().unwrap().push(raw_msg.clone());
                    let body = json!({ "breakpoints": [] });
                    self.send_response(req_seq, command, true, Some(body), None)?;
                } else if has_jdtls {
                    if let Some(ref mut jw) = *self.jdtls_writer.lock().unwrap() {
                        let _ = write_dap_message(jw, raw_msg);
                    }
                } else {
                    let body = json!({ "breakpoints": [] });
                    self.send_response(req_seq, command, true, Some(body), None)?;
                }
            }
            _ => {
                let has_jdtls = self.jdtls_writer.lock().unwrap().is_some();
                if has_jdtls {
                    if let Some(ref mut jw) = *self.jdtls_writer.lock().unwrap() {
                        let _ = write_dap_message(jw, raw_msg);
                    }
                } else {
                    match command {
                        "configurationDone"
                        | "setExceptionBreakpoints"
                        | "pause"
                        | "continue"
                        | "next"
                        | "stepIn"
                        | "stepOut" => {
                            self.send_response(req_seq, command, true, None, None)?;
                        }
                        "stackTrace" => {
                            let body = json!({ "stackFrames": [], "totalFrames": 0 });
                            self.send_response(req_seq, command, true, Some(body), None)?;
                        }
                        "scopes" => {
                            let body = json!({ "scopes": [] });
                            self.send_response(req_seq, command, true, Some(body), None)?;
                        }
                        "variables" => {
                            let body = json!({ "variables": [] });
                            self.send_response(req_seq, command, true, Some(body), None)?;
                        }
                        _ => {
                            dap_log(&format!("Unhandled request in standalone mode: {command}"));
                            self.send_response(req_seq, command, true, None, None)?;
                        }
                    }
                }
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
        // Drop any JDTLS connection since the orchestrator session does not debug a child directly
        let _ = self.jdtls_writer.lock().unwrap().take();
        let _ = self.jdtls_reader.lock().unwrap().take();

        if !self.initialized_sent.swap(true, Ordering::SeqCst) {
            self.send_event("initialized", None)?;
        }

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

        let leader_pid = std::process::id();

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
                        "skipBuild": true,
                        "groupLeaderPid": leader_pid
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

        // Send DAP `process` and `thread` events so Zed recognizes an active process/thread,
        // which activates the "Terminate All Threads" / Stop button in Zed's debug panel.
        self.send(&json!({
            "seq": self.next_seq(),
            "type": "event",
            "event": "process",
            "body": {
                "name": format!("Group: {group_name}"),
                "systemProcessId": leader_pid,
                "isLocalProcess": true,
                "startMethod": "launch"
            }
        }))?;
        self.send(&json!({
            "seq": self.next_seq(),
            "type": "event",
            "event": "thread",
            "body": {
                "reason": "started",
                "threadId": 1
            }
        }))?;

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

        let group_leader_pid = args
            .get("groupLeaderPid")
            .and_then(Value::as_i64)
            .or_else(|| args.get("groupLeaderPid").and_then(Value::as_u64).map(|v| v as i64));

        let has_jdtls = self.jdtls_writer.lock().unwrap().is_some();
        let jdwp_port = if has_jdtls {
            Some(allocate_free_port()?)
        } else {
            args.get("debugPort")
                .and_then(Value::as_u64)
                .map(|p| p as u16)
        };

        let mut plan = plan::build(&project, entry, &options, jdwp_port, has_jdtls)?;
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

        if let Some(port) = jdwp_port {
            if has_jdtls {
                let mut attached = false;
                let mut last_err = String::new();
                for attempt in 0..20 {
                    thread::sleep(Duration::from_millis(50));
                    let attach_seq = self.jdtls_seq.fetch_add(1, Ordering::SeqCst);
                    let attach_req = json!({
                        "seq": attach_seq,
                        "type": "request",
                        "command": "attach",
                        "arguments": {
                            "hostName": "127.0.0.1",
                            "port": port,
                            "projectName": entry.module
                        }
                    });
                    {
                        let mut jw_guard = self.jdtls_writer.lock().unwrap();
                        if let Some(ref mut jw) = *jw_guard {
                            if let Err(e) = write_dap_message(jw, &attach_req) {
                                last_err = format!("Failed to send attach: {e}");
                                continue;
                            }
                        }
                    }

                    // Read until attach response
                    let mut jr_guard = self.jdtls_reader.lock().unwrap();
                    if let Some(ref mut jr) = *jr_guard {
                        let mut got_attach_resp = false;
                        while let Ok(Some(msg)) = read_dap_message(jr) {
                            dap_log(&format!("JDTLS attach handshake: {:?}", msg));
                            let msg_type = msg.get("type").and_then(Value::as_str).unwrap_or("");
                            if msg_type == "event" {
                                let event = msg.get("event").and_then(Value::as_str).unwrap_or("");
                                if event == "initialized" {
                                    if !self.initialized_sent.swap(true, Ordering::SeqCst) {
                                        self.send_event("initialized", None)?;
                                    }
                                }
                            } else if msg_type == "response" {
                                let cmd = msg.get("command").and_then(Value::as_str).unwrap_or("");
                                if cmd == "attach" {
                                    got_attach_resp = true;
                                    if msg.get("success").and_then(Value::as_bool).unwrap_or(false) {
                                        attached = true;
                                    } else {
                                        last_err = msg
                                            .get("message")
                                            .and_then(Value::as_str)
                                            .unwrap_or("Attach failed")
                                            .to_string();
                                    }
                                    break;
                                }
                            }
                        }
                        if attached || got_attach_resp {
                            if attached {
                                dap_log(&format!(
                                    "Attached to JDWP port {port} on attempt {}",
                                    attempt + 1
                                ));
                                break;
                            }
                        }
                    }
                }

                if !attached {
                    let err_msg = format!("Failed to attach JDTLS to JDWP port {port}: {last_err}");
                    dap_log(&err_msg);
                    self.send_response(req_seq, "launch", false, None, Some(&err_msg))?;
                    return Ok(());
                }

                // Replay any buffered breakpoints
                let pending = self.pending_breakpoints.lock().unwrap().drain(..).collect::<Vec<_>>();
                if !pending.is_empty() {
                    let mut jw_guard = self.jdtls_writer.lock().unwrap();
                    if let Some(ref mut jw) = *jw_guard {
                        for bp_req in pending {
                            dap_log(&format!("Replaying buffered breakpoint: {:?}", bp_req));
                            let _ = write_dap_message(jw, &bp_req);
                        }
                    }
                }

                // Spawn JDTLS forwarder thread
                let jr_opt = self.jdtls_reader.lock().unwrap().take();
                if let Some(mut jr) = jr_opt {
                    let writer = self.writer.clone();
                    let is_stopped = self.is_stopped.clone();
                    let terminated = self.terminated.clone();
                    let display_name_clone = display_name.clone();
                    let last_stopped_thread_id = self.last_stopped_thread_id.clone();
                    let seq = self.seq.clone();
                    thread::spawn(move || {
                        while let Ok(Some(mut val)) = read_dap_message(&mut jr) {
                            if terminated.load(Ordering::SeqCst) {
                                break;
                            }
                            dap_log(&format!("JDTLS -> {}", val));
                            let msg_type = val.get("type").and_then(Value::as_str).unwrap_or("");
                            if msg_type == "event" {
                                let event = val.get("event").and_then(Value::as_str).unwrap_or("");
                                if event == "stopped" {
                                    is_stopped.store(true, Ordering::SeqCst);
                                    if let Some(tid) = val
                                        .get("body")
                                        .and_then(|b| b.get("threadId"))
                                        .and_then(Value::as_i64)
                                    {
                                        last_stopped_thread_id.store(tid, Ordering::SeqCst);
                                    }
                                } else if event == "continued" {
                                    is_stopped.store(false, Ordering::SeqCst);
                                } else if event == "thread" {
                                    // Suppress internal JVM daemon/worker thread lifecycle events from JDTLS
                                    // so Zed doesn't accumulate 40+ threads in its session thread map.
                                    // A thread map with length > 1 breaks Zed's single-thread session naming fallback,
                                    // causing Zed to display "(child)".
                                    continue;
                                }
                            } else if msg_type == "response" {
                                let cmd = val.get("command").and_then(Value::as_str).unwrap_or("");
                                if cmd == "threads" {
                                    let stopped_tid = last_stopped_thread_id.load(Ordering::SeqCst);
                                    let final_tid = if stopped_tid > 0 {
                                        stopped_tid
                                    } else {
                                        val.get("body")
                                            .and_then(|b| b.get("threads"))
                                            .and_then(Value::as_array)
                                            .and_then(|arr| arr.first())
                                            .and_then(|t| t.get("id"))
                                            .and_then(Value::as_i64)
                                            .unwrap_or(1)
                                    };
                                    // Return ONLY the stopped/active thread, with its name set to the service display name.
                                    // Because threads.len() == 1, Zed uses threads[0].name as the session name,
                                    // ensuring the session displays its service name (e.g. "SystemApplication")
                                    // instead of falling back to "(child)".
                                    val["body"] = json!({
                                        "threads": [
                                            {
                                                "id": final_tid,
                                                "name": display_name_clone
                                            }
                                        ]
                                    });
                                }
                            }
                            val["seq"] = json!(seq.fetch_add(1, Ordering::SeqCst));
                            let mut w = writer.lock().unwrap();
                            let _ = write_dap_message(&mut *w, &val);
                        }
                    });
                }
            }
        }

        // DAP process event
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
            let seq = self.seq.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().flatten() {
                    let mut w = writer.lock().unwrap();
                    let _ = write_dap_message(
                        &mut *w,
                        &json!({
                            "seq": seq.fetch_add(1, Ordering::SeqCst),
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
            let seq = self.seq.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    let mut w = writer.lock().unwrap();
                    let _ = write_dap_message(
                        &mut *w,
                        &json!({
                            "seq": seq.fetch_add(1, Ordering::SeqCst),
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

        // Wait thread & Watchdog
        let child_arc = self.child.clone();
        let writer = self.writer.clone();
        let terminated = self.terminated.clone();
        let entry_id_arc = self.entry_id.clone();
        let root = self.root.clone();
        let seq = self.seq.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(300));
                if terminated.load(Ordering::SeqCst) {
                    break;
                }

                // If this child session was spawned by a group orchestrator, monitor parent's liveness
                if let Some(leader_pid) = group_leader_pid {
                    let alive = unsafe { libc::kill(leader_pid as i32, 0) == 0 };
                    if !alive {
                        dap_log(&format!(
                            "Group leader PID {leader_pid} exited; terminating child session"
                        ));
                        terminated.store(true, Ordering::SeqCst);
                        let mut guard = child_arc.lock().unwrap();
                        if let Some(mut child) = guard.take() {
                            let pid = child.id() as i32;
                            unsafe {
                                libc::kill(-pid, libc::SIGTERM);
                            }
                            let _ = child.kill();
                            dap_log(&format!("Watchdog killed child JVM PID {pid}"));
                        }
                        if let Some(ref id) = *entry_id_arc.lock().unwrap() {
                            if let Ok(dir) = runtime::state_dir(&root) {
                                let _ = std::fs::remove_file(dir.join(format!("{}.pid", runtime::hash(id))));
                            }
                        }
                        let mut w = writer.lock().unwrap();
                        let _ = write_dap_message(
                            &mut *w,
                            &json!({
                                "seq": seq.fetch_add(1, Ordering::SeqCst),
                                "type": "event",
                                "event": "output",
                                "body": {
                                    "category": "stdout",
                                    "output": "[Java Launcher] Group orchestrator ended; shutting down.\n",
                                }
                            }),
                        );
                        let _ = write_dap_message(
                            &mut *w,
                            &json!({
                                "seq": seq.fetch_add(1, Ordering::SeqCst),
                                "type": "event",
                                "event": "exited",
                                "body": {
                                    "exitCode": 0,
                                }
                            }),
                        );
                        let _ = write_dap_message(
                            &mut *w,
                            &json!({
                                "seq": seq.fetch_add(1, Ordering::SeqCst),
                                "type": "event",
                                "event": "terminated",
                            }),
                        );
                        break;
                    }
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
                                    "seq": seq.fetch_add(1, Ordering::SeqCst),
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
                                    "seq": seq.fetch_add(1, Ordering::SeqCst),
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
                                    "seq": seq.fetch_add(1, Ordering::SeqCst),
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
            None,
        );

        let dummy_threads_req = json!({
            "seq": 1,
            "type": "request",
            "command": "threads"
        });

        // Before launch without initial_name: threads returns "main"
        server
            .handle_request(1, "threads", &json!({}), &dummy_threads_req)
            .unwrap();
        let msgs = output.lock().unwrap().clone();
        let mut cursor = Cursor::new(msgs);
        let resp = read_dap_message(&mut cursor).unwrap().unwrap();
        assert_eq!(resp["body"]["threads"][0]["name"], "main");

        // When initialized with initial_name (e.g. from CLI --name passed by extension)
        let output2 = Arc::new(Mutex::new(Vec::new()));
        let writer2 = SharedWriter(output2.clone());
        let server2 = DapServer::new(
            writer2,
            PathBuf::from("/nonexistent"),
            PathBuf::from("/nonexistent"),
            Some("WarehouseApplication".to_string()),
        );
        server2
            .handle_request(1, "threads", &json!({}), &dummy_threads_req)
            .unwrap();
        let msgs2 = output2.lock().unwrap().clone();
        let mut cursor2 = Cursor::new(msgs2);
        let resp2 = read_dap_message(&mut cursor2).unwrap().unwrap();
        assert_eq!(resp2["body"]["threads"][0]["name"], "WarehouseApplication");

        // Manually set display name as done during launch
        *server.display_name.lock().unwrap() = Some("SystemApplication".to_string());
        output.lock().unwrap().clear();
        server
            .handle_request(2, "threads", &json!({}), &dummy_threads_req)
            .unwrap();
        let msgs = output.lock().unwrap().clone();
        let mut cursor = Cursor::new(msgs);
        let resp = read_dap_message(&mut cursor).unwrap().unwrap();
        assert_eq!(resp["body"]["threads"][0]["name"], "SystemApplication");

        // For group session
        *server.display_name.lock().unwrap() = Some("Group: uis".to_string());
        output.lock().unwrap().clear();
        server
            .handle_request(3, "threads", &json!({}), &dummy_threads_req)
            .unwrap();
        let msgs = output.lock().unwrap().clone();
        let mut cursor = Cursor::new(msgs);
        let resp = read_dap_message(&mut cursor).unwrap().unwrap();
        assert_eq!(resp["body"]["threads"][0]["name"], "Group: uis");

        // When a breakpoint hits on thread 141 in SystemApplication:
        *server.display_name.lock().unwrap() = Some("SystemApplication".to_string());
        server.last_stopped_thread_id.store(141, Ordering::SeqCst);
        output.lock().unwrap().clear();
        server
            .handle_request(4, "threads", &json!({}), &dummy_threads_req)
            .unwrap();
        let msgs = output.lock().unwrap().clone();
        let mut cursor = Cursor::new(msgs);
        let resp = read_dap_message(&mut cursor).unwrap().unwrap();
        let threads = resp["body"]["threads"].as_array().unwrap();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0]["id"], 141);
        assert_eq!(threads[0]["name"], "SystemApplication");
    }

    #[test]
    fn test_hex_decode_and_allocate_port() {
        let path = "/Users/river/IdeaProjects/uis";
        let hex: String = path
            .as_bytes()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        let decoded = hex_decode(&hex).expect("valid hex");
        assert_eq!(String::from_utf8(decoded).unwrap(), path);

        let port = allocate_free_port().expect("free port");
        assert!(port > 1024);
    }

    #[test]
    fn test_terminate_threads_request_handling() {
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
            None,
        );

        let term_req = json!({
            "seq": 10,
            "type": "request",
            "command": "terminateThreads",
            "arguments": {
                "threadIds": [1]
            }
        });

        let should_break = server
            .handle_request(10, "terminateThreads", &term_req["arguments"], &term_req)
            .unwrap();
        assert!(should_break);
        assert!(server.terminated.load(Ordering::SeqCst));

        let msgs = output.lock().unwrap().clone();
        let mut cursor = Cursor::new(msgs);

        // 1. Response for terminateThreads
        let resp = read_dap_message(&mut cursor).unwrap().unwrap();
        assert_eq!(resp["command"], "terminateThreads");
        assert_eq!(resp["success"], true);

        // 2. Event exited
        let exited = read_dap_message(&mut cursor).unwrap().unwrap();
        assert_eq!(exited["event"], "exited");
        assert_eq!(exited["body"]["exitCode"], 0);

        // 3. Event terminated
        let term = read_dap_message(&mut cursor).unwrap().unwrap();
        assert_eq!(term["event"], "terminated");
    }
}
