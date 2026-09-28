use crate::{
    model::{Config, EntryKind, Project},
    runtime,
};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{io, path::Path, process::Command, time::Duration};

enum Action {
    Quit,
    Run,
    Stop,
    Restart,
    Logs,
    Profile(String),
    GroupUp,
    GroupDown,
    Help,
    Refresh,
}
#[derive(Clone)]
enum Row {
    Entry(String),
    Group(String),
}
pub fn open(root: &Path, config_path: &Path, project: &Project, config: &Config) -> Result<()> {
    let mut app = App::new(root, config_path, project, config);
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = loop {
        terminal.draw(|f| draw(f, &app))?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(action) = app.handle(key.code, key.modifiers) {
                        match action {
                            Action::Quit => break Ok(()),
                            other => app.execute(other),
                        }
                    }
                }
            }
        }
        app.poll();
    };
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

struct App<'a> {
    root: &'a Path,
    config_path: &'a Path,
    project: &'a Project,
    config: &'a Config,
    query: String,
    selected: usize,
    rows: Vec<Row>,
    message: String,
    help: bool,
    profile_mode: bool,
    profile_input: String,
}
impl<'a> App<'a> {
    fn new(
        root: &'a Path,
        config_path: &'a Path,
        project: &'a Project,
        config: &'a Config,
    ) -> Self {
        let mut app = Self {
            root,
            config_path,
            project,
            config,
            query: String::new(),
            selected: 0,
            rows: vec![],
            message: "r run  s stop  R restart  l logs  p profile  g/G group  / search  q quit"
                .into(),
            help: false,
            profile_mode: false,
            profile_input: String::new(),
        };
        app.refresh_rows();
        app
    }
    fn refresh_rows(&mut self) {
        let matcher = SkimMatcherV2::default();
        let q = self.query.trim();
        let mut rows = vec![];
        for name in self.config.groups.keys() {
            if q.is_empty() || matcher.fuzzy_match(name, q).is_some() {
                rows.push(Row::Group(name.clone()));
            }
        }
        for entry in self
            .project
            .entries
            .iter()
            .filter(|e| e.kind == EntryKind::SpringBoot)
        {
            let label = format!("{} {}", entry.module, entry.class);
            if q.is_empty()
                || matcher.fuzzy_match(&label, q).is_some()
                || matcher.fuzzy_match(&entry.id, q).is_some()
            {
                rows.push(Row::Entry(entry.id.clone()));
            }
        }
        self.rows = rows;
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len().saturating_sub(1);
        }
    }
    fn handle(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Option<Action> {
        if self.profile_mode {
            match code {
                KeyCode::Esc => self.profile_mode = false,
                KeyCode::Enter => {
                    let profile = std::mem::take(&mut self.profile_input);
                    self.profile_mode = false;
                    return Some(Action::Profile(profile));
                }
                KeyCode::Backspace => {
                    self.profile_input.pop();
                }
                KeyCode::Char(c) if !c.is_whitespace() => self.profile_input.push(c),
                _ => {}
            }
            return None;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
            KeyCode::Char('?') => Some(Action::Help),
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.rows.len() {
                    self.selected += 1;
                }
                None
            }
            KeyCode::Char('r') => Some(Action::Run),
            KeyCode::Char('s') => Some(Action::Stop),
            KeyCode::Char('R') => Some(Action::Restart),
            KeyCode::Char('l') => Some(Action::Logs),
            KeyCode::Char('p') => {
                self.profile_mode = true;
                self.profile_input.clear();
                None
            }
            KeyCode::Char('g') => Some(Action::GroupUp),
            KeyCode::Char('G') => Some(Action::GroupDown),
            KeyCode::Char('/') => {
                self.query.clear();
                self.refresh_rows();
                None
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.refresh_rows();
                None
            }
            KeyCode::Char(c) => {
                self.query.push(c);
                self.refresh_rows();
                None
            }
            KeyCode::F(1) => Some(Action::Help),
            KeyCode::F(5) => Some(Action::Refresh),
            _ => None,
        }
    }
    fn selected_row(&self) -> Option<Row> {
        self.rows.get(self.selected).cloned()
    }
    fn execute(&mut self, action: Action) {
        if matches!(action, Action::Help) {
            self.help = !self.help;
            return;
        }
        if matches!(action, Action::Refresh) {
            self.poll();
            self.message = "Refreshed".into();
            return;
        }
        let Some(row) = self.selected_row() else {
            self.message = "No selection".into();
            return;
        };
        match (action, row) {
            (Action::Run, Row::Entry(id)) => self.launch("start", &id),
            (Action::Stop, Row::Entry(id)) => self.launch("stop", &id),
            (Action::Restart, Row::Entry(id)) => self.launch("restart", &id),
            (Action::Logs, Row::Entry(id)) => self.logs(&id),
            (Action::Profile(profile), Row::Entry(id)) => self.profile(&id, &profile),
            (Action::GroupUp, Row::Group(name)) => self.launch_group("up", &name),
            (Action::GroupDown, Row::Group(name)) => self.launch_group("down", &name),
            (_, Row::Group(_)) => self.message = "Select an application for that action".into(),
            (_, Row::Entry(_)) => self.message = "Select a group for group actions".into(),
        }
    }
    fn launcher(&self) -> Command {
        let mut c = Command::new(std::env::current_exe().unwrap_or_default());
        c.arg("--project")
            .arg(self.root)
            .arg("--config")
            .arg(self.config_path);
        c
    }
    fn launch(&mut self, action: &str, id: &str) {
        let output = self.launcher().arg(action).arg(id).output();
        self.message = summarize(action, output);
    }
    fn launch_group(&mut self, action: &str, name: &str) {
        let output = self.launcher().args(["group", action, name]).output();
        self.message = summarize(&format!("group {action}"), output);
    }
    fn profile(&mut self, id: &str, profile: &str) {
        let output = self.launcher().args(["profile", id, profile]).output();
        self.message = summarize("profile", output);
    }
    fn logs(&mut self, id: &str) {
        match runtime::request(self.root, id, "status") {
            Ok(status) => self.message = format!("Log: {}", status.log),
            Err(_) => self.message = format!("No live log for {id}; start it first"),
        }
    }
    fn poll(&mut self) {
        let _ = runtime::statuses(self.root);
    }
    fn status_label(&self, id: &str) -> String {
        runtime::request(self.root, id, "status")
            .map(|s| s.phase)
            .unwrap_or_else(|_| "stopped".into())
    }
}

fn summarize(action: &str, output: std::io::Result<std::process::Output>) -> String {
    match output {
        Ok(out) if out.status.success() => format!(
            "{action}: {}",
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("ok")
        ),
        Ok(out) => format!(
            "{action} failed: {}",
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or("unknown error")
        ),
        Err(e) => format!("{action} failed: {e}"),
    }
}

fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Java Launcher",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::raw(app.root.display().to_string()),
        ]))
        .block(Block::default().borders(Borders::ALL).title("Zed")),
        chunks[0],
    );
    let items: Vec<ListItem> = app
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let selected = i == app.selected;
            let (label, style) = match row {
                Row::Group(name) => (format!("GROUP  {name}"), Style::default().fg(Color::Cyan)),
                Row::Entry(id) => {
                    let entry = app.project.entries.iter().find(|e| e.id == *id);
                    let module = entry.map(|e| e.module.as_str()).unwrap_or("?");
                    let class = entry.map(|e| e.class.as_str()).unwrap_or(id);
                    let phase = app.status_label(id);
                    let color = if phase == "stopped" {
                        Color::Gray
                    } else {
                        Color::Green
                    };
                    (
                        format!("{phase:<10} {module:<12} {class}"),
                        Style::default().fg(color),
                    )
                }
            };
            let style = if selected {
                style.add_modifier(Modifier::REVERSED)
            } else {
                style
            };
            ListItem::new(label).style(style)
        })
        .collect();
    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title(format!(
            "Search: {}",
            if app.query.is_empty() {
                "type to filter".into()
            } else {
                app.query.clone()
            }
        ))),
        chunks[1],
    );
    let footer = if app.profile_mode {
        format!("Profile: {} (Enter save, Esc cancel)", app.profile_input)
    } else {
        app.message.clone()
    };
    frame.render_widget(
        Paragraph::new(footer)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("Status")),
        chunks[2],
    );
    if app.help {
        let area = centered(frame.area(), 60, 12);
        frame.render_widget(Paragraph::new("r Run   s Stop   R Restart\nl Logs  p Profile\ng Group up   G Group down\n/ clear search   Backspace edit search\nF5 refresh   q quit").block(Block::default().borders(Borders::ALL).title("Help")), area);
    }
}
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
