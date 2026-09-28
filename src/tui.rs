use crate::{
    config,
    model::{Config, EntryKind, Project},
    plan, runtime, scan,
};
use anyhow::{bail, Context, Result};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Search,
    ProfileInput,
    LogViewer,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    SpringBootOnly,
    AppsOnly,
    All,
}

impl FilterKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SpringBootOnly => "Spring Boot Only",
            Self::AppsOnly => "Applications (Spring Boot + Main)",
            Self::All => "All Entries (including Tests)",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::SpringBootOnly => Self::AppsOnly,
            Self::AppsOnly => Self::All,
            Self::All => Self::SpringBootOnly,
        }
    }
}

#[derive(Clone, Debug)]
pub enum RowItem {
    Group {
        name: String,
        member_count: usize,
        running_count: usize,
    },
    Entry {
        id: String,
        kind: EntryKind,
        module: String,
        class: String,
    },
}

enum WorkerMsg {
    StatusUpdate(Vec<runtime::Status>),
    ActionResult {
        action: String,
        result: Result<String>,
    },
    ReloadResult(Result<(Project, Config)>),
}

struct LogViewerState {
    entry_id: String,
    path: PathBuf,
    lines: Vec<String>,
    scroll: usize,
    follow: bool,
    last_len: u64,
    fullscreen: bool,
    visible_height: usize,
}

impl LogViewerState {
    pub fn max_scroll(&self) -> usize {
        let vh = if self.visible_height == 0 { 25 } else { self.visible_height };
        self.lines.len().saturating_sub(vh)
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.follow = false;
        let max = self.max_scroll();
        if self.scroll > max {
            self.scroll = max;
        }
        self.scroll = self.scroll.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        let max = self.max_scroll();
        self.scroll = self.scroll.saturating_add(amount);
        if self.scroll >= max {
            self.scroll = max;
            self.follow = true;
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.follow = false;
        self.scroll = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.follow = true;
        self.scroll = self.max_scroll();
    }
}

struct App {
    root: PathBuf,
    config_path: PathBuf,
    project: Project,
    config: Config,
    mode: Mode,
    filter_kind: FilterKind,
    query: String,
    selected: usize,
    list_state: ListState,
    rows: Vec<RowItem>,
    statuses: HashMap<String, runtime::Status>,
    message: String,
    message_is_error: bool,
    busy: Option<String>,
    spinner_idx: usize,
    tick_count: usize,
    profile_input: String,
    profile_target: Option<String>,
    log_viewer: Option<LogViewerState>,
    tx: Sender<WorkerMsg>,
    rx: Receiver<WorkerMsg>,
}

impl App {
    fn new(root: &Path, config_path: &Path, project: Project, config: Config) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut app = Self {
            root: root.to_path_buf(),
            config_path: config_path.to_path_buf(),
            project,
            config,
            mode: Mode::Normal,
            filter_kind: FilterKind::SpringBootOnly,
            query: String::new(),
            selected: 0,
            list_state: ListState::default(),
            rows: Vec::new(),
            statuses: HashMap::new(),
            message: "Ready. Press 'r' to run, 's' to stop, '/' to search, '?' for help.".into(),
            message_is_error: false,
            busy: None,
            spinner_idx: 0,
            tick_count: 0,
            profile_input: String::new(),
            profile_target: None,
            log_viewer: None,
            tx,
            rx,
        };
        app.spawn_status_poll();
        app.refresh_rows();
        app
    }

    fn spawn_status_poll(&self) {
        let root = self.root.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            if let Ok(st) = runtime::statuses(&root) {
                let _ = tx.send(WorkerMsg::StatusUpdate(st));
            }
        });
    }

    fn refresh_rows(&mut self) {
        let q = self.query.trim();
        let matcher = SkimMatcherV2::default();
        let mut scored_rows = Vec::new();

        // 1. Groups
        for (name, items) in &self.config.groups {
            let member_count = items.len();
            let running_count = items
                .iter()
                .filter(|i| {
                    self.statuses
                        .get(&i.entry)
                        .map_or(false, |s| s.phase == "running" || s.phase == "between_steps")
                })
                .count();

            let row = RowItem::Group {
                name: name.clone(),
                member_count,
                running_count,
            };

            if q.is_empty() {
                scored_rows.push((1000, row));
            } else if let Some(score) = matcher.fuzzy_match(name, q) {
                scored_rows.push((score + 100, row));
            }
        }

        // 2. Entries
        for entry in &self.project.entries {
            let matches_filter = match self.filter_kind {
                FilterKind::SpringBootOnly => entry.kind == EntryKind::SpringBoot,
                FilterKind::AppsOnly => !entry.kind.is_test(),
                FilterKind::All => true,
            };
            if !matches_filter {
                continue;
            }

            let row = RowItem::Entry {
                id: entry.id.clone(),
                kind: entry.kind.clone(),
                module: entry.module.clone(),
                class: entry.class.clone(),
            };

            if q.is_empty() {
                scored_rows.push((0, row));
            } else {
                let label = format!("{} {} {}", entry.id, entry.module, entry.class);
                if let Some(score) = matcher.fuzzy_match(&label, q) {
                    scored_rows.push((score, row));
                }
            }
        }

        if !q.is_empty() {
            scored_rows.sort_by(|a, b| b.0.cmp(&a.0));
        }

        self.rows = scored_rows.into_iter().map(|(_, r)| r).collect();

        if self.rows.is_empty() {
            self.selected = 0;
            self.list_state.select(None);
        } else {
            if self.selected >= self.rows.len() {
                self.selected = self.rows.len().saturating_sub(1);
            }
            self.list_state.select(Some(self.selected));
        }
    }

    fn selected_row(&self) -> Option<&RowItem> {
        self.rows.get(self.selected)
    }

    fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        self.spinner_idx = self.spinner_idx.wrapping_add(1);

        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                WorkerMsg::StatusUpdate(list) => {
                    self.statuses.clear();
                    for s in list {
                        self.statuses.insert(s.entry.clone(), s);
                    }
                    self.refresh_rows();
                }
                WorkerMsg::ActionResult { action, result } => {
                    self.busy = None;
                    match result {
                        Ok(msg) => {
                            self.message = msg;
                            self.message_is_error = false;
                        }
                        Err(e) => {
                            self.message = format!("{action} failed: {e:#}");
                            self.message_is_error = true;
                        }
                    }
                    self.spawn_status_poll();
                }
                WorkerMsg::ReloadResult(res) => {
                    self.busy = None;
                    match res {
                        Ok((proj, cfg)) => {
                            self.project = proj;
                            self.config = cfg;
                            self.refresh_rows();
                            self.message = "Project and config reloaded successfully.".into();
                            self.message_is_error = false;
                        }
                        Err(e) => {
                            self.message = format!("Reload failed: {e:#}");
                            self.message_is_error = true;
                        }
                    }
                    self.spawn_status_poll();
                }
            }
        }

        // Auto-refresh log viewer in real time if open (every ~300ms)
        if self.mode == Mode::LogViewer && self.tick_count % 3 == 0 {
            self.auto_refresh_log();
        }

        // Periodic poll every ~2s
        if self.tick_count % 20 == 0 && self.busy.is_none() {
            self.spawn_status_poll();
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::Search => self.handle_search(key),
            Mode::ProfileInput => self.handle_profile_input(key),
            Mode::LogViewer => self.handle_log_viewer(key),
            Mode::Help => self.handle_help(key),
        }
    }

    fn handle_normal(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return true;
        }

        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
            }
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.mode = Mode::Help;
            }
            KeyCode::Tab => {
                self.filter_kind = self.filter_kind.next();
                self.refresh_rows();
                self.message = format!("Filter switched to: {}", self.filter_kind.name());
                self.message_is_error = false;
            }
            KeyCode::F(5) => {
                self.trigger_reload();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.rows.is_empty() {
                    self.selected = self.selected.saturating_sub(1);
                    self.list_state.select(Some(self.selected));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.rows.is_empty() && self.selected + 1 < self.rows.len() {
                    self.selected += 1;
                    self.list_state.select(Some(self.selected));
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                if !self.rows.is_empty() {
                    self.selected = 0;
                    self.list_state.select(Some(self.selected));
                }
            }
            KeyCode::End | KeyCode::Char('G') => {
                if !self.rows.is_empty() {
                    self.selected = self.rows.len().saturating_sub(1);
                    self.list_state.select(Some(self.selected));
                }
            }
            KeyCode::PageUp => {
                if !self.rows.is_empty() {
                    self.selected = self.selected.saturating_sub(10);
                    self.list_state.select(Some(self.selected));
                }
            }
            KeyCode::PageDown => {
                if !self.rows.is_empty() {
                    self.selected = (self.selected + 10).min(self.rows.len().saturating_sub(1));
                    self.list_state.select(Some(self.selected));
                }
            }
            KeyCode::Char('r') | KeyCode::Enter => {
                self.trigger_run(false);
            }
            KeyCode::Char('d') => {
                self.trigger_run(true);
            }
            KeyCode::Char('s') => {
                self.trigger_stop();
            }
            KeyCode::Char('R') => {
                self.trigger_restart();
            }
            KeyCode::Char(' ') => {
                self.trigger_toggle();
            }
            KeyCode::Char('l') => {
                self.open_log_viewer();
            }
            KeyCode::Char('o') => {
                self.open_selected_in_zed();
            }
            KeyCode::Char('p') => {
                self.open_profile_input();
            }
            KeyCode::Backspace => {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.refresh_rows();
                    self.message = "Search query cleared.".into();
                    self.message_is_error = false;
                }
            }
            _ => {}
        }
        false
    }

    fn handle_search(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return true;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.refresh_rows();
            }
            KeyCode::Char(c) => {
                self.query.push(c);
                self.refresh_rows();
            }
            KeyCode::Up => {
                if !self.rows.is_empty() {
                    self.selected = self.selected.saturating_sub(1);
                    self.list_state.select(Some(self.selected));
                }
            }
            KeyCode::Down => {
                if !self.rows.is_empty() && self.selected + 1 < self.rows.len() {
                    self.selected += 1;
                    self.list_state.select(Some(self.selected));
                }
            }
            _ => {}
        }
        false
    }

    fn handle_profile_input(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.profile_target = None;
            }
            KeyCode::Enter => {
                self.save_profile();
            }
            KeyCode::Backspace => {
                self.profile_input.pop();
            }
            KeyCode::Char(c) if !c.is_whitespace() => {
                self.profile_input.push(c);
            }
            _ => {}
        }
        false
    }

    fn handle_log_viewer(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('l') => {
                self.mode = Mode::Normal;
                self.log_viewer = None;
            }
            KeyCode::Char('m') => {
                if let Some(lv) = &mut self.log_viewer {
                    lv.fullscreen = !lv.fullscreen;
                }
            }
            KeyCode::Char('o') => {
                if let Some(lv) = &self.log_viewer {
                    match runtime::open_in_zed(&lv.path) {
                        Ok(()) => {
                            self.message = format!("Opened log in Zed: {}", lv.path.display());
                            self.message_is_error = false;
                        }
                        Err(e) => {
                            self.message = format!("Failed to open in Zed: {e}");
                            self.message_is_error = true;
                        }
                    }
                }
            }
            KeyCode::Char('f') => {
                if let Some(lv) = &mut self.log_viewer {
                    lv.follow = !lv.follow;
                    if lv.follow {
                        lv.scroll = lv.max_scroll();
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(lv) = &mut self.log_viewer {
                    lv.scroll_up(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(lv) = &mut self.log_viewer {
                    lv.scroll_down(1);
                }
            }
            KeyCode::PageUp => {
                if let Some(lv) = &mut self.log_viewer {
                    let step = lv.visible_height.max(5).saturating_sub(2);
                    lv.scroll_up(step);
                }
            }
            KeyCode::PageDown => {
                if let Some(lv) = &mut self.log_viewer {
                    let step = lv.visible_height.max(5).saturating_sub(2);
                    lv.scroll_down(step);
                }
            }
            KeyCode::Home => {
                if let Some(lv) = &mut self.log_viewer {
                    lv.scroll_to_top();
                }
            }
            KeyCode::End => {
                if let Some(lv) = &mut self.log_viewer {
                    lv.scroll_to_bottom();
                }
            }
            KeyCode::Char('r') => {
                self.reload_log_content();
            }
            _ => {}
        }
        false
    }

    fn handle_help(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc
            | KeyCode::Char('q')
            | KeyCode::Char('?')
            | KeyCode::F(1)
            | KeyCode::Enter => {
                self.mode = Mode::Normal;
            }
            _ => {}
        }
        false
    }

    fn trigger_run(&mut self, debug: bool) {
        if self.busy.is_some() {
            self.message = "Another action is currently in progress...".into();
            self.message_is_error = true;
            return;
        }

        let Some(row) = self.selected_row().cloned() else {
            self.message = "No item selected.".into();
            self.message_is_error = true;
            return;
        };

        match row {
            RowItem::Entry { id, .. } => {
                let debug_port = if debug { Some(5005) } else { None };
                self.busy = Some(format!(
                    "Starting {id}{}...",
                    if debug { " with debug port 5005" } else { "" }
                ));
                self.statuses
                    .entry(id.clone())
                    .or_insert_with(|| runtime::Status {
                        entry: id.clone(),
                        phase: "starting...".into(),
                        supervisor_pid: 0,
                        child_pid: None,
                        started_at: 0,
                        debug_port,
                        log: String::new(),
                        watch_pid: None,
                    })
                    .phase = "starting...".into();

                let root = self.root.clone();
                let config_path = self.config_path.clone();
                let tx = self.tx.clone();
                let id_clone = id.clone();

                thread::spawn(move || {
                    let res = runtime::start(&root, &config_path, &id_clone, false, debug_port, false, None);
                    let result = match res {
                        Ok(st) => Ok(format!(
                            "Started {} (Supervisor PID: {}{})",
                            id_clone,
                            st.supervisor_pid,
                            if let Some(p) = st.debug_port {
                                format!(", Debug Port: {p}")
                            } else {
                                String::new()
                            }
                        )),
                        Err(e) => Err(e),
                    };
                    let _ = tx.send(WorkerMsg::ActionResult {
                        action: format!("start {id_clone}"),
                        result,
                    });
                    if let Ok(st) = runtime::statuses(&root) {
                        let _ = tx.send(WorkerMsg::StatusUpdate(st));
                    }
                });
            }
            RowItem::Group { name, .. } => {
                if debug {
                    self.message = "Debug mode is only supported for single applications.".into();
                    self.message_is_error = true;
                    return;
                }
                self.busy = Some(format!("Starting group '{name}'..."));
                let root = self.root.clone();
                let config_path = self.config_path.clone();
                let project = self.project.clone();
                let config = self.config.clone();
                let tx = self.tx.clone();
                let group_name = name.clone();

                thread::spawn(move || {
                    let res = (|| -> Result<String> {
                        let items = config
                            .groups
                            .get(&group_name)
                            .with_context(|| format!("Unknown group {group_name}"))?;
                        let enabled: Vec<_> = items.iter().filter(|i| i.enabled).collect();
                        if enabled.is_empty() {
                            bail!("Group has no enabled items");
                        }
                        let mut ids = std::collections::BTreeSet::new();
                        for item in &enabled {
                            let entry = project.entry(&item.entry)?;
                            if entry.kind.is_test() {
                                bail!("Group member is a test, not an application: {}", entry.id);
                            }
                            if !ids.insert(&entry.id) {
                                bail!("Duplicate group member {}", entry.id);
                            }
                            if runtime::request(&root, &entry.id, "status").is_ok() {
                                bail!("Group member already running: {}", entry.id);
                            }
                            let o = config.options(&entry.id);
                            plan::build(&project, entry, &o, None, false)?;
                            config::environment(&root, &o)?;
                        }
                        let mut started = vec![];
                        for item in enabled {
                            if item.delay_ms > 0 {
                                thread::sleep(Duration::from_millis(item.delay_ms));
                            }
                            match runtime::start(&root, &config_path, &item.entry, false, None, false, None) {
                                Ok(_) => started.push(item.entry.clone()),
                                Err(e) => {
                                    for prev in started.iter().rev() {
                                        let _ = runtime::stop(&root, prev);
                                    }
                                    return Err(e);
                                }
                            }
                        }
                        Ok(format!(
                            "Group '{group_name}' started successfully ({} applications).",
                            started.len()
                        ))
                    })();

                    let _ = tx.send(WorkerMsg::ActionResult {
                        action: format!("group up {group_name}"),
                        result: res,
                    });
                    if let Ok(st) = runtime::statuses(&root) {
                        let _ = tx.send(WorkerMsg::StatusUpdate(st));
                    }
                });
            }
        }
    }

    fn trigger_stop(&mut self) {
        if self.busy.is_some() {
            self.message = "Another action is currently in progress...".into();
            self.message_is_error = true;
            return;
        }

        let Some(row) = self.selected_row().cloned() else {
            self.message = "No item selected.".into();
            self.message_is_error = true;
            return;
        };

        match row {
            RowItem::Entry { id, .. } => {
                self.busy = Some(format!("Stopping {id}..."));
                if let Some(status) = self.statuses.get_mut(&id) {
                    status.phase = "stopping...".into();
                }

                let root = self.root.clone();
                let tx = self.tx.clone();
                let id_clone = id.clone();

                thread::spawn(move || {
                    let res = runtime::stop(&root, &id_clone)
                        .map(|_| format!("Stopped application {id_clone}."));
                    let _ = tx.send(WorkerMsg::ActionResult {
                        action: format!("stop {id_clone}"),
                        result: res,
                    });
                    if let Ok(st) = runtime::statuses(&root) {
                        let _ = tx.send(WorkerMsg::StatusUpdate(st));
                    }
                });
            }
            RowItem::Group { name, .. } => {
                self.busy = Some(format!("Stopping group '{name}'..."));
                let root = self.root.clone();
                let config = self.config.clone();
                let tx = self.tx.clone();
                let group_name = name.clone();

                thread::spawn(move || {
                    let res = (|| -> Result<String> {
                        let items = config
                            .groups
                            .get(&group_name)
                            .with_context(|| format!("Unknown group {group_name}"))?;
                        let mut stopped = 0;
                        for item in items.iter().rev() {
                            if runtime::request(&root, &item.entry, "status").is_ok() {
                                runtime::stop(&root, &item.entry)?;
                                stopped += 1;
                            }
                        }
                        Ok(format!("Group '{group_name}' stopped ({stopped} applications)."))
                    })();

                    let _ = tx.send(WorkerMsg::ActionResult {
                        action: format!("group down {group_name}"),
                        result: res,
                    });
                    if let Ok(st) = runtime::statuses(&root) {
                        let _ = tx.send(WorkerMsg::StatusUpdate(st));
                    }
                });
            }
        }
    }

    fn trigger_restart(&mut self) {
        if self.busy.is_some() {
            self.message = "Another action is currently in progress...".into();
            self.message_is_error = true;
            return;
        }

        let Some(row) = self.selected_row().cloned() else {
            self.message = "No item selected.".into();
            self.message_is_error = true;
            return;
        };

        match row {
            RowItem::Entry { id, .. } => {
                self.busy = Some(format!("Restarting {id}..."));
                if let Some(status) = self.statuses.get_mut(&id) {
                    status.phase = "restarting...".into();
                }

                let root = self.root.clone();
                let config_path = self.config_path.clone();
                let tx = self.tx.clone();
                let id_clone = id.clone();

                thread::spawn(move || {
                    let _ = runtime::stop(&root, &id_clone);
                    let res = runtime::start(&root, &config_path, &id_clone, false, None, false, None);
                    let result = match res {
                        Ok(st) => Ok(format!(
                            "Restarted {} (Supervisor PID: {})",
                            id_clone, st.supervisor_pid
                        )),
                        Err(e) => Err(e),
                    };
                    let _ = tx.send(WorkerMsg::ActionResult {
                        action: format!("restart {id_clone}"),
                        result,
                    });
                    if let Ok(st) = runtime::statuses(&root) {
                        let _ = tx.send(WorkerMsg::StatusUpdate(st));
                    }
                });
            }
            RowItem::Group { .. } => {
                self.message = "Restarting whole group: please stop then start.".into();
                self.message_is_error = false;
            }
        }
    }

    fn trigger_toggle(&mut self) {
        let Some(row) = self.selected_row() else {
            return;
        };
        match row {
            RowItem::Entry { id, .. } => {
                let is_running = self
                    .statuses
                    .get(id)
                    .map_or(false, |s| s.phase == "running" || s.phase == "between_steps");
                if is_running {
                    self.trigger_stop();
                } else {
                    self.trigger_run(false);
                }
            }
            RowItem::Group { running_count, .. } => {
                if *running_count > 0 {
                    self.trigger_stop();
                } else {
                    self.trigger_run(false);
                }
            }
        }
    }

    fn trigger_reload(&mut self) {
        if self.busy.is_some() {
            self.message = "Another action is currently in progress...".into();
            self.message_is_error = true;
            return;
        }

        self.busy = Some("Reloading project and configuration...".into());
        let root = self.root.clone();
        let config_path = self.config_path.clone();
        let tx = self.tx.clone();

        thread::spawn(move || {
            let res = (|| -> Result<(Project, Config)> {
                let project = scan::scan(&root)?;
                let config = config::load(&config_path)?;
                Ok((project, config))
            })();
            let _ = tx.send(WorkerMsg::ReloadResult(res));
            if let Ok(st) = runtime::statuses(&root) {
                let _ = tx.send(WorkerMsg::StatusUpdate(st));
            }
        });
    }

    fn open_profile_input(&mut self) {
        let Some(row) = self.selected_row().cloned() else {
            self.message = "No item selected.".into();
            self.message_is_error = true;
            return;
        };

        match row {
            RowItem::Entry { id, kind, .. } => {
                if kind != EntryKind::SpringBoot {
                    self.message = "Spring profile is only supported for Spring Boot entries.".into();
                    self.message_is_error = true;
                    return;
                }
                let current_profile = self
                    .config
                    .entries
                    .get(&id)
                    .and_then(|o| o.spring_profile.clone())
                    .unwrap_or_default();
                self.profile_input = current_profile;
                self.profile_target = Some(id);
                self.mode = Mode::ProfileInput;
            }
            RowItem::Group { .. } => {
                self.message = "Select an application to edit Spring profile.".into();
                self.message_is_error = true;
            }
        }
    }

    fn save_profile(&mut self) {
        let Some(id) = self.profile_target.take() else {
            self.mode = Mode::Normal;
            return;
        };

        let profile = self.profile_input.trim().to_string();
        if profile.is_empty() {
            self.config.entries.entry(id.clone()).or_default().spring_profile = None;
        } else {
            self.config.entries.entry(id.clone()).or_default().spring_profile = Some(profile.clone());
        }

        match config::atomic_json(&self.config_path, &self.config) {
            Ok(()) => {
                self.message = if profile.is_empty() {
                    format!("Cleared Spring profile for {id}.")
                } else {
                    format!("Saved Spring profile for {id}: {profile}")
                };
                self.message_is_error = false;
            }
            Err(e) => {
                self.message = format!("Failed to save config: {e:#}");
                self.message_is_error = true;
            }
        }
        self.mode = Mode::Normal;
    }

    fn open_log_viewer(&mut self) {
        let Some(row) = self.selected_row() else {
            self.message = "No item selected.".into();
            self.message_is_error = true;
            return;
        };

        match row {
            RowItem::Entry { id, .. } => {
                let log_path = self
                    .statuses
                    .get(id)
                    .map(|s| PathBuf::from(&s.log))
                    .unwrap_or_else(|| {
                        runtime::state_dir(&self.root)
                            .map(|d| d.join(format!("{}.log", runtime::hash(id))))
                            .unwrap_or_default()
                    });

                let lines = if log_path.exists() {
                    let content = fs::read_to_string(&log_path).unwrap_or_default();
                    content
                        .lines()
                        .rev()
                        .take(2000)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .map(String::from)
                        .collect()
                } else {
                    vec![format!(
                        "Log file does not exist yet: {}",
                        log_path.display()
                    )]
                };

                let last_len = fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);
                let mut state = LogViewerState {
                    entry_id: id.clone(),
                    path: log_path,
                    lines,
                    scroll: 0,
                    follow: true,
                    last_len,
                    fullscreen: false,
                    visible_height: 25,
                };
                state.scroll = state.max_scroll();
                self.log_viewer = Some(state);
                self.mode = Mode::LogViewer;
            }
            RowItem::Group { .. } => {
                self.message = "Select an application to view its logs.".into();
                self.message_is_error = true;
            }
        }
    }

    fn open_selected_in_zed(&mut self) {
        let Some(row) = self.selected_row() else {
            self.message = "No item selected.".into();
            self.message_is_error = true;
            return;
        };

        match row {
            RowItem::Entry { id, .. } => {
                let log_path = self
                    .statuses
                    .get(id)
                    .map(|s| PathBuf::from(&s.log))
                    .unwrap_or_else(|| {
                        runtime::state_dir(&self.root)
                            .map(|d| d.join(format!("{}.log", runtime::hash(id))))
                            .unwrap_or_default()
                    });

                match runtime::open_in_zed(&log_path) {
                    Ok(()) => {
                        self.message = format!("Opened log in Zed: {}", log_path.display());
                        self.message_is_error = false;
                    }
                    Err(e) => {
                        self.message = format!("Failed to open in Zed: {e}");
                        self.message_is_error = true;
                    }
                }
            }
            RowItem::Group { .. } => {
                self.message = "Select a specific entry to open its log in Zed.".into();
                self.message_is_error = true;
            }
        }
    }

    fn auto_refresh_log(&mut self) {
        let Some(lv) = &mut self.log_viewer else {
            return;
        };
        if !lv.path.exists() {
            return;
        }
        let current_len = fs::metadata(&lv.path).map(|m| m.len()).unwrap_or(0);
        if current_len != lv.last_len {
            lv.last_len = current_len;
            let content = fs::read_to_string(&lv.path).unwrap_or_default();
            lv.lines = content
                .lines()
                .rev()
                .take(2000)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(String::from)
                .collect();
            if lv.follow {
                lv.scroll = lv.max_scroll();
            }
        }
    }

    fn reload_log_content(&mut self) {
        if let Some(lv) = &mut self.log_viewer {
            if lv.path.exists() {
                lv.last_len = fs::metadata(&lv.path).map(|m| m.len()).unwrap_or(0);
                let content = fs::read_to_string(&lv.path).unwrap_or_default();
                lv.lines = content
                    .lines()
                    .rev()
                    .take(2000)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .map(String::from)
                    .collect();
                if lv.follow {
                    lv.scroll = lv.max_scroll();
                }
                self.message = "Log content refreshed.".into();
                self.message_is_error = false;
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(3), // Search & Filter
                Constraint::Min(6),    // Main list
                Constraint::Length(3), // Footer / Status
            ])
            .split(frame.area());

        self.draw_header(frame, chunks[0]);
        self.draw_search_bar(frame, chunks[1]);
        self.draw_list(frame, chunks[2]);
        self.draw_footer(frame, chunks[3]);

        match self.mode {
            Mode::Help => self.draw_help_modal(frame),
            Mode::ProfileInput => self.draw_profile_modal(frame),
            Mode::LogViewer => self.draw_log_modal(frame),
            _ => {}
        }
    }

    fn draw_header(&self, frame: &mut Frame, area: Rect) {
        let running_apps = self
            .statuses
            .values()
            .filter(|s| s.phase == "running" || s.phase == "between_steps")
            .count();
        let total_apps = self
            .project
            .entries
            .iter()
            .filter(|e| !e.kind.is_test())
            .count();
        let group_count = self.config.groups.len();

        let header_line = Line::from(vec![
            Span::styled(
                " Zed Java Launcher ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                self.root.display().to_string(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  |  "),
            Span::styled(
                format!("{running_apps}/{total_apps} apps running"),
                Style::default().fg(if running_apps > 0 {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
            Span::raw("  |  "),
            Span::styled(
                format!("{group_count} groups"),
                Style::default().fg(Color::Yellow),
            ),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Project ");
        frame.render_widget(Paragraph::new(header_line).block(block), area);
    }

    fn draw_search_bar(&self, frame: &mut Frame, area: Rect) {
        let (title, border_color) = match self.mode {
            Mode::Search => (" Search [SEARCH MODE: Enter confirm, Esc exit] ", Color::Yellow),
            _ => (" Search [Press '/' to search, Backspace to clear] ", Color::DarkGray),
        };

        let filter_badge = format!(" [Filter: {} (Tab to switch)] ", self.filter_kind.name());

        let line = if self.query.is_empty() {
            if self.mode == Mode::Search {
                Line::from(vec![
                    Span::styled(
                        "▌",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::RAPID_BLINK),
                    ),
                    Span::styled(
                        " type to fuzzy search modules, classes, and groups...",
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            } else {
                Line::from(vec![
                    Span::styled(
                        "Press '/' to search...",
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            }
        } else {
            Line::from(vec![
                Span::styled(
                    &self.query,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                if self.mode == Mode::Search {
                    Span::styled("▌", Style::default().fg(Color::Yellow))
                } else {
                    Span::raw("")
                },
                Span::styled(
                    format!(" ({} matches)", self.rows.len()),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title)
            .title(Line::from(filter_badge).alignment(Alignment::Right));

        frame.render_widget(Paragraph::new(line).block(block), area);
    }

    fn draw_list(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .rows
            .iter()
            .map(|row| match row {
                RowItem::Group {
                    name,
                    member_count,
                    running_count,
                } => {
                    let badge = Span::styled(
                        " ◆ GROUP     ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    );
                    let name_span = Span::styled(
                        format!("{:<28} ", name),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    );
                    let info_span = Span::styled(
                        format!(
                            "{member_count} members ({} running)",
                            if *running_count > 0 {
                                format!("{running_count}")
                            } else {
                                "none".into()
                            }
                        ),
                        Style::default().fg(if *running_count > 0 {
                            Color::Green
                        } else {
                            Color::DarkGray
                        }),
                    );
                    ListItem::new(Line::from(vec![badge, name_span, info_span]))
                }
                RowItem::Entry {
                    id, module, class, ..
                } => {
                    let status = self.statuses.get(id);
                    let phase = status.map(|s| s.phase.as_str()).unwrap_or("stopped");

                    let (badge, badge_style) = match phase {
                        "running" => (
                            " ● RUNNING   ",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        "stopping" | "stopping..." => (
                            " ▼ STOPPING  ",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                        ),
                        "preparing" | "waiting_for_build" | "building" | "starting..." => (
                            " ▲ STARTING  ",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        "between_steps" => (
                            " ◉ RELOAD    ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        _ => (
                            " ○ STOPPED   ",
                            Style::default().fg(Color::DarkGray),
                        ),
                    };

                    let module_span = Span::styled(
                        format!("{:<14} ", module),
                        Style::default().fg(Color::LightBlue),
                    );
                    let class_span = Span::styled(
                        format!("{:<32} ", class),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    );

                    let mut spans = vec![
                        Span::styled(badge, badge_style),
                        module_span,
                        class_span,
                    ];

                    if let Some(port) = status.and_then(|s| s.debug_port) {
                        spans.push(Span::styled(
                            format!(" [debug:{port}]"),
                            Style::default().fg(Color::Magenta),
                        ));
                    }

                    if let Some(prof) = self
                        .config
                        .entries
                        .get(id)
                        .and_then(|o| o.spring_profile.as_ref())
                    {
                        spans.push(Span::styled(
                            format!(" [profile:{prof}]"),
                            Style::default().fg(Color::Yellow),
                        ));
                    }

                    if let Some(pid) = status.and_then(|s| s.child_pid.or(Some(s.supervisor_pid))) {
                        if pid > 0 && phase == "running" {
                            spans.push(Span::styled(
                                format!(" (pid:{pid})"),
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                    }

                    ListItem::new(Line::from(spans))
                }
            })
            .collect();

        let list_widget = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Applications & Groups ")
                    .border_style(Style::default().fg(Color::White)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(40, 50, 75))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list_widget, area, &mut self.list_state);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let status_color = if self.message_is_error {
            Color::LightRed
        } else {
            Color::LightGreen
        };

        let message_line = if let Some(ref busy) = self.busy {
            Line::from(vec![
                Span::styled(
                    format!("{} ", SPINNER_FRAMES[self.spinner_idx % SPINNER_FRAMES.len()]),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(busy, Style::default().fg(Color::Yellow)),
            ])
        } else {
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&self.message, Style::default().fg(status_color)),
            ])
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(
                Line::from(
                    " [r]un [s]top [R]estart [d]ebug [l]ogs [o]zed [p]rofile [space]toggle [/]search [F5]reload [?]help [q]uit "
                )
                .alignment(Alignment::Right),
            );

        frame.render_widget(Paragraph::new(message_line).block(block), area);
    }

    fn draw_help_modal(&self, frame: &mut Frame) {
        let area = centered_rect(65, 65, frame.area());
        frame.render_widget(Clear, area);

        let text = vec![
            Line::from(Span::styled(
                "KEYBOARD SHORTCUTS & USAGE",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Navigation:     ", Style::default().fg(Color::Yellow)),
                Span::raw("j/k or ↑/↓ (move)  |  g/G or Home/End (top/bottom)"),
            ]),
            Line::from(vec![
                Span::styled("                ", Style::default()),
                Span::raw("PgUp/PgDown (jump 10)  |  Tab (cycle filter)"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Execution:      ", Style::default().fg(Color::Yellow)),
                Span::raw("r / Enter  - Start application or group"),
            ]),
            Line::from(vec![
                Span::styled("                ", Style::default()),
                Span::raw("s          - Stop application or group"),
            ]),
            Line::from(vec![
                Span::styled("                ", Style::default()),
                Span::raw("R          - Restart application"),
            ]),
            Line::from(vec![
                Span::styled("                ", Style::default()),
                Span::raw("d          - Start with Debug port (5005)"),
            ]),
            Line::from(vec![
                Span::styled("                ", Style::default()),
                Span::raw("Space      - Toggle start/stop state"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Management:     ", Style::default().fg(Color::Yellow)),
                Span::raw("l          - View tail logs in modal viewer"),
            ]),
            Line::from(vec![
                Span::styled("                ", Style::default()),
                Span::raw("o          - Open log file directly in Zed editor"),
            ]),
            Line::from(vec![
                Span::styled("                ", Style::default()),
                Span::raw("p          - Configure active Spring profile"),
            ]),
            Line::from(vec![
                Span::styled("                ", Style::default()),
                Span::raw("F5         - Full project rescan & config reload"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Search & Modal: ", Style::default().fg(Color::Yellow)),
                Span::raw("/          - Enter fuzzy search mode"),
            ]),
            Line::from(vec![
                Span::styled("                ", Style::default()),
                Span::raw("m          - Toggle fullscreen in Log Viewer"),
            ]),
            Line::from(vec![
                Span::styled("                ", Style::default()),
                Span::raw("f          - Toggle follow (auto-scroll) in Log Viewer"),
            ]),
            Line::from(vec![
                Span::styled("                ", Style::default()),
                Span::raw("Esc / q    - Close modal dialogs / Exit"),
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .title(" Help (Press Esc or q to close) ");

        frame.render_widget(Paragraph::new(text).block(block).wrap(Wrap { trim: false }), area);
    }

    fn draw_profile_modal(&self, frame: &mut Frame) {
        let area = centered_rect(55, 25, frame.area());
        frame.render_widget(Clear, area);

        let target = self.profile_target.as_deref().unwrap_or("unknown");
        let text = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("Current value: "),
                Span::styled(
                    if self.profile_input.is_empty() {
                        "<default/none>".to_string()
                    } else {
                        self.profile_input.clone()
                    },
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("Input: "),
                Span::styled(
                    format!("{}_", self.profile_input),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Press Enter to save, Esc to cancel. Leave empty to clear.",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .title(format!(" Set Spring Profile for {target} "));

        frame.render_widget(Paragraph::new(text).block(block), area);
    }

    fn draw_log_modal(&mut self, frame: &mut Frame) {
        let is_fullscreen = self
            .log_viewer
            .as_ref()
            .map(|lv| lv.fullscreen)
            .unwrap_or(false);
        let area = if is_fullscreen {
            frame.area()
        } else {
            centered_rect(90, 85, frame.area())
        };
        frame.render_widget(Clear, area);

        let Some(lv) = &mut self.log_viewer else {
            return;
        };

        let visible_height = area.height.saturating_sub(2) as usize;
        lv.visible_height = visible_height;
        let total_lines = lv.lines.len();

        let max_scroll = lv.max_scroll();
        if lv.follow {
            lv.scroll = max_scroll;
        } else if lv.scroll > max_scroll {
            lv.scroll = max_scroll;
        }
        let current_scroll = lv.scroll;

        let end_idx = (current_scroll + visible_height).min(total_lines);
        let slice = if current_scroll < total_lines {
            &lv.lines[current_scroll..end_idx]
        } else {
            &[]
        };

        let items: Vec<ListItem> = slice
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let line_num = current_scroll + i + 1;
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{line_num:>4} │ "),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(line),
                ]))
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(if lv.fullscreen {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::LightBlue)
            })
            .title(format!(
                " Logs: {} ({}) [Line {}/{}] ",
                lv.entry_id,
                lv.path.display(),
                if total_lines == 0 { 0 } else { (current_scroll + slice.len()).min(total_lines) },
                total_lines
            ))
            .title(
                Line::from(vec![
                    if lv.follow {
                        Span::styled(
                            "[f] Follow: ON (LIVE)  ",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Span::styled(
                            "[f] Follow: PAUSED  ",
                            Style::default().fg(Color::Yellow),
                        )
                    },
                    Span::styled(
                        if lv.fullscreen { "[m] Windowed  " } else { "[m] Fullscreen  " },
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        "[o] Open in Zed  ",
                        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("[↑/↓/PgUp/PgDn] Scroll  [r] Refresh  [q/Esc/l] Close "),
                ])
                .alignment(Alignment::Right),
            );

        frame.render_widget(List::new(items).block(block), area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
    }
}

pub fn open(root: &Path, config_path: &Path, project: &Project, config: &Config) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;

    let _cleanup_guard = TerminalCleanup;

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
        default_hook(info);
    }));

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(root, config_path, project.clone(), config.clone());

    loop {
        terminal.draw(|f| app.draw(f))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.handle_key(key) {
                        break;
                    }
                }
            }
        }

        app.tick();
    }

    Ok(())
}
