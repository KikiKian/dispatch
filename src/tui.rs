use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, stdout};
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{CrosstermBackend, Terminal},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, TableState},
    Frame,
};
use sysinfo::{Pid, System, SystemExt};

use crate::{
    auto_balance, eco_mode, gaming_mode, kill_process, performance_mode, read_tasks,
    refreshed_system, score_process, Process,
};

// Caps the output panel's scrollback so a long session doesn't grow unbounded.
const MAX_OUTPUT_LINES: usize = 200;

// How often continuous eco/performance mode re-applies itself while toggled
// on, so newly spawned processes (or affinity Windows reset) get caught too
// instead of only being pinned by a one-shot sweep at toggle time.
const CONTINUOUS_REAPPLY_INTERVAL: Duration = Duration::from_secs(5);

const HELP_TEXT: &str = "up/down select  space priority  e toggle eco  p toggle performance  \
a auto-balance  g gaming  k kill selected  r refresh  q quit";

#[derive(Clone, Copy, PartialEq)]
enum ContinuousMode {
    Eco,
    Performance,
}

// All UI state lives here. Render code only ever reads from this; state
// changes go through the methods below so `draw` stays dumb.
struct App {
    processes: Vec<Process>,
    sys: System,
    priority: HashSet<usize>,
    // `score_process` prints a line per process (via `cpu_usage_of`), so scores
    // are cached here and only recomputed on explicit refresh/toggle instead of
    // every frame — otherwise the render loop would flood stdout at 60fps.
    scores: HashMap<usize, u32>,
    // Tracks both the selected row and the scroll offset, so the process table
    // scrolls to keep the selection in view instead of clipping past the area.
    table_state: TableState,
    status: String,
    // Action results (core pins, kills, mode summaries) land here instead of
    // stdout, so they show up in the Output widget instead of corrupting the
    // alternate screen.
    output: VecDeque<String>,
    // When set, the main loop re-runs this mode every `CONTINUOUS_REAPPLY_INTERVAL`
    // instead of relying on a single sweep applied at toggle time.
    continuous_mode: Option<ContinuousMode>,
    last_applied: Instant,
}

impl App {
    fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        let mut app = App {
            processes: Vec::new(),
            sys: System::new(),
            priority: HashSet::new(),
            scores: HashMap::new(),
            table_state,
            status: "Ready".to_string(),
            output: VecDeque::new(),
            continuous_mode: None,
            last_applied: Instant::now(),
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        self.processes = read_tasks().into_values().collect();
        self.processes.sort_by_key(|p| p.pid);
        self.sys = refreshed_system();

        let len = self.processes.len();
        if self.table_state.selected().is_none_or(|s| s >= len) {
            self.table_state.select(if len == 0 { None } else { Some(len - 1) });
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
        let current = self.table_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len - 1) as usize;
        self.table_state.select(Some(next));
    }

    fn toggle_priority(&mut self) {
        if let Some(process) = self.selected_process() {
            let pid = process.pid;
            if !self.priority.remove(&pid) {
                self.priority.insert(pid);
            }
        }
        self.recompute_scores();
    }

    fn selected_process(&self) -> Option<&Process> {
        self.table_state.selected().and_then(|i| self.processes.get(i))
    }

    fn log(&mut self, lines: Vec<String>) {
        for line in lines {
            if self.output.len() >= MAX_OUTPUT_LINES {
                self.output.pop_front();
            }
            self.output.push_back(line);
        }
    }

    // Re-runs whichever continuous mode is active. Called both on toggle and
    // periodically from the main loop.
    fn apply_continuous_mode(&mut self) {
        let mut log = Vec::new();
        match self.continuous_mode {
            Some(ContinuousMode::Eco) => eco_mode(&mut log),
            Some(ContinuousMode::Performance) => performance_mode(&self.priority, &mut log),
            None => return,
        }
        self.log(log);
        self.last_applied = Instant::now();
    }
}

// What the main loop should do after handling a key press.
struct Outcome {
    quit: bool,
    // Mode actions (eco/performance/auto-balance/gaming/kill) print straight to
    // stdout, so the caller must force a full redraw afterward to wipe that output.
    redraw: bool,
}

const STAY: Outcome = Outcome { quit: false, redraw: false };
const REDRAW: Outcome = Outcome { quit: false, redraw: true };
const QUIT: Outcome = Outcome { quit: true, redraw: false };

fn handle_key(app: &mut App, key: KeyCode) -> Outcome {
    match key {
        KeyCode::Char('q') => QUIT,
        KeyCode::Up => {
            app.move_selection(-1);
            STAY
        }
        KeyCode::Down => {
            app.move_selection(1);
            STAY
        }
        KeyCode::Char(' ') => {
            app.toggle_priority();
            REDRAW
        }
        KeyCode::Char('r') => {
            app.refresh();
            app.status = "Refreshed process list".to_string();
            REDRAW
        }
        KeyCode::Char('e') => {
            if app.continuous_mode == Some(ContinuousMode::Eco) {
                app.continuous_mode = None;
                app.status = "Eco mode stopped".to_string();
            } else {
                app.continuous_mode = Some(ContinuousMode::Eco);
                app.apply_continuous_mode();
                app.status = "Eco mode running (re-applies every 5s)".to_string();
            }
            REDRAW
        }
        KeyCode::Char('p') => {
            if app.continuous_mode == Some(ContinuousMode::Performance) {
                app.continuous_mode = None;
                app.status = "Performance mode stopped".to_string();
            } else {
                app.continuous_mode = Some(ContinuousMode::Performance);
                app.apply_continuous_mode();
                app.status = format!(
                    "Performance mode running ({} priority pids, re-applies every 5s)",
                    app.priority.len()
                );
            }
            REDRAW
        }
        KeyCode::Char('a') => {
            let mut log = Vec::new();
            auto_balance(&mut log);
            app.log(log);
            app.status = "Auto-balanced across all cores".to_string();
            REDRAW
        }
        KeyCode::Char('g') => {
            if let Some(process) = app.selected_process() {
                let (pid, name) = (process.pid, process.name.clone());
                let mut log = Vec::new();
                gaming_mode(Pid::from(pid), &mut log);
                app.log(log);
                app.status = format!("Gaming mode: reserved cores for {} ({})", name, pid);
            }
            REDRAW
        }
        KeyCode::Char('k') => {
            if let Some(process) = app.selected_process() {
                let (pid, name) = (process.pid, process.name.clone());
                let mut log = Vec::new();
                kill_process(Pid::from(pid), &mut log);
                app.log(log);
                app.status = format!("Killed {} ({})", name, pid);
                app.refresh();
            }
            REDRAW
        }
        _ => STAY,
    }
}

fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(8),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, chunks[0], app);
    draw_process_table(frame, chunks[1], app);
    draw_output(frame, chunks[2], app);
    draw_help(frame, chunks[3], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let mode_label = match app.continuous_mode {
        Some(ContinuousMode::Eco) => " — eco mode: ON",
        Some(ContinuousMode::Performance) => " — performance mode: ON",
        None => "",
    };
    let header = Paragraph::new(format!(
        "{} processes — {} marked priority{}",
        app.processes.len(),
        app.priority.len(),
        mode_label,
    ))
    .block(Block::default().title(" Dispatch ").borders(Borders::ALL));

    frame.render_widget(header, area);
}

fn draw_process_table(frame: &mut Frame, area: Rect, app: &mut App) {
    let selected = app.table_state.selected();
    let rows: Vec<Row> = app
        .processes
        .iter()
        .enumerate()
        .map(|(i, process)| process_row(app, i == selected.unwrap_or(usize::MAX), process))
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

    // Stateful render: ratatui scrolls the visible window to keep
    // `table_state.selected()` in view instead of clipping it.
    frame.render_stateful_widget(table, area, &mut app.table_state);
}

fn process_row(app: &App, is_selected: bool, process: &Process) -> Row<'static> {
    let score = app.scores.get(&process.pid).copied().unwrap_or(0);
    let is_priority = app.priority.contains(&process.pid);

    let mut style = Style::default();
    if is_priority {
        style = style.fg(Color::Yellow);
    }
    if is_selected {
        style = style.add_modifier(Modifier::REVERSED);
    }

    Row::new(vec![
        Cell::from(process.pid.to_string()),
        Cell::from(process.name.clone()),
        Cell::from(score.to_string()),
        Cell::from(if is_priority { "*" } else { "" }),
    ])
    .style(style)
}

fn draw_output(frame: &mut Frame, area: Rect, app: &App) {
    // Always show the trailing lines, like `tail -f`, instead of tracking
    // separate scroll state.
    let visible = area.height.saturating_sub(2) as usize;
    let items: Vec<ListItem> = app
        .output
        .iter()
        .rev()
        .take(visible)
        .rev()
        .map(|line| ListItem::new(line.clone()))
        .collect();

    let list = List::new(items).block(Block::default().title(" Output ").borders(Borders::ALL));
    frame.render_widget(list, area);
}

fn draw_help(frame: &mut Frame, area: Rect, app: &App) {
    let help = Paragraph::new(format!("{} | {}", HELP_TEXT, app.status))
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(help, area);
}

// Restores the terminal on every exit path (normal return, early `?`, or panic
// unwind) since none of those run code placed after the main loop otherwise.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
    }
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
        terminal.draw(|frame| draw(frame, &mut app))?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let outcome = handle_key(&mut app, key.code);
                    if outcome.redraw {
                        terminal.clear()?;
                    }
                    if outcome.quit {
                        break;
                    }
                }
            }
        }

        if app.continuous_mode.is_some() && app.last_applied.elapsed() >= CONTINUOUS_REAPPLY_INTERVAL {
            app.apply_continuous_mode();
        }
    }

    Ok(())
}
