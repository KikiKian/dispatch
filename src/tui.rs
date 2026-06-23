use std::collections::{HashMap, HashSet};
use std::io::{self, stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{CrosstermBackend, Terminal},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};
use sysinfo::{Pid, System};

use super::{
    auto_balance, eco_mode, gaming_mode, kill_low_priority, performance_mode, read_tasks,
    refreshed_system, score_process, Process,
};

const DEFAULT_KILL_THRESHOLD: u32 = 500;
const KILL_THRESHOLD_STEP: u32 = 100;

// Restores the terminal on every exit path (normal return, early `?`, or panic
// unwind) since none of those run code placed after the main loop otherwise.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
    }
}

struct App {
    processes: Vec<Process>,
    sys: System,
    priority: HashSet<usize>,
    // `score_process` prints a line per process (via `cpu_usage_of`), so scores
    // are cached here and only recomputed on explicit refresh/toggle instead of
    // every frame — otherwise the render loop would flood stdout at 60fps.
    scores: HashMap<usize, u32>,
    selected: usize,
    kill_threshold: u32,
    status: String,
}

impl App {
    fn new() -> Self {
        let mut app = App {
            processes: Vec::new(),
            sys: refreshed_system(),
            priority: HashSet::new(),
            scores: HashMap::new(),
            selected: 0,
            kill_threshold: DEFAULT_KILL_THRESHOLD,
            status: "Ready".to_string(),
        };
        app.processes = read_tasks().into_values().collect();
        app.processes.sort_by_key(|p| p.pid);
        app.recompute_scores();
        app
    }

    fn refresh(&mut self) {
        self.processes = read_tasks().into_values().collect();
        self.processes.sort_by_key(|p| p.pid);
        self.sys = refreshed_system();
        if self.selected >= self.processes.len() {
            self.selected = self.processes.len().saturating_sub(1);
        }
        self.recompute_scores();
    }

    fn recompute_scores(&mut self) {
        self.scores = self
            .processes
            .iter()
            .map(|p| (p.pid, score_process(&self.sys, &self.priority, Pid::from(p.pid))))
            .collect();
    }

    fn move_selection(&mut self, delta: i32) {
        if self.processes.is_empty() {
            return;
        }
        let len = self.processes.len() as i32;
        self.selected = (self.selected as i32 + delta).clamp(0, len - 1) as usize;
    }

    fn toggle_priority(&mut self) {
        if let Some(process) = self.processes.get(self.selected) {
            if !self.priority.remove(&process.pid) {
                self.priority.insert(process.pid);
            }
        }
        self.recompute_scores();
    }
}

// Handles one key press. Returns (should_quit, noisy). `noisy` actions call
// into `direct_process`/`kill_low_priority`, which print directly to stdout,
// so the caller forces a full redraw afterward to wipe that output.
fn handle_key(app: &mut App, key: KeyCode) -> (bool, bool) {
    match key {
        KeyCode::Char('q') => (true, false),
        KeyCode::Up => {
            app.move_selection(-1);
            (false, false)
        }
        KeyCode::Down => {
            app.move_selection(1);
            (false, false)
        }
        KeyCode::Char(' ') => {
            app.toggle_priority();
            (false, true)
        }
        KeyCode::Char('r') => {
            app.refresh();
            app.status = "Refreshed process list".to_string();
            (false, true)
        }
        KeyCode::Char('+') => {
            app.kill_threshold = app.kill_threshold.saturating_add(KILL_THRESHOLD_STEP);
            (false, false)
        }
        KeyCode::Char('-') => {
            app.kill_threshold = app.kill_threshold.saturating_sub(KILL_THRESHOLD_STEP);
            (false, false)
        }
        KeyCode::Char('e') => {
            eco_mode();
            app.status = "Eco mode applied".to_string();
            (false, true)
        }
        KeyCode::Char('p') => {
            performance_mode(&app.priority);
            app.status = format!(
                "Performance mode applied ({} priority pids)",
                app.priority.len()
            );
            (false, true)
        }
        KeyCode::Char('a') => {
            auto_balance();
            app.status = "Auto-balanced across all cores".to_string();
            (false, true)
        }
        KeyCode::Char('g') => {
            if let Some(process) = app.processes.get(app.selected) {
                let (pid, name) = (process.pid, process.name.clone());
                gaming_mode(Pid::from(pid));
                app.status = format!("Gaming mode: reserved cores for {} ({})", name, pid);
            }
            (false, true)
        }
        KeyCode::Char('k') => {
            kill_low_priority(app.kill_threshold);
            app.status = format!("Killed processes scoring below {}", app.kill_threshold);
            app.refresh();
            (false, true)
        }
        _ => (false, false),
    }
}

fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let header = Paragraph::new(format!(
        "{} processes — {} marked priority — kill threshold {}",
        app.processes.len(),
        app.priority.len(),
        app.kill_threshold
    ))
    .block(Block::default().title(" Dispatch ").borders(Borders::ALL));
    frame.render_widget(header, chunks[0]);

    let rows: Vec<Row> = app
        .processes
        .iter()
        .enumerate()
        .map(|(i, process)| {
            let score = app.scores.get(&process.pid).copied().unwrap_or(0);
            let is_priority = app.priority.contains(&process.pid);

            let mut style = Style::default();
            if is_priority {
                style = style.fg(Color::Yellow);
            }
            if i == app.selected {
                style = style.add_modifier(Modifier::REVERSED);
            }

            Row::new(vec![
                Cell::from(process.pid.to_string()),
                Cell::from(process.name.clone()),
                Cell::from(score.to_string()),
                Cell::from(if is_priority { "*" } else { "" }),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(3),
        ],
    )
    .header(Row::new(vec!["PID", "Name", "Score", "Pri"]).style(Style::default().add_modifier(Modifier::BOLD)))
    .block(Block::default().title(" Processes ").borders(Borders::ALL));
    frame.render_widget(table, chunks[1]);

    let help = Paragraph::new(format!(
        "up/down select  space priority  e eco  p performance  a auto-balance  g gaming  k kill  +/- threshold  r refresh  q quit | {}",
        app.status
    ))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[2]);
}

pub fn tui() -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    terminal.clear()?;
    loop {
        terminal.draw(|frame| ui(frame, &app))?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let (should_quit, noisy) = handle_key(&mut app, key.code);
                    if noisy {
                        terminal.clear()?;
                    }
                    if should_quit {
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}
