use std::io::{self, stdout};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::{CrosstermBackend, Terminal},
    widgets::{Block, Borders, Paragraph},
};

fn tui() -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    // App state & render loop
    let mut should_quit = false;
    while !should_quit {
        terminal.draw(|frame| {
            let size = frame.area();
            
            // Create a simple bordered block
            let block = Block::default()
                .title(" Ratatui Template ")
                .borders(Borders::ALL);

            // Create a paragraph widget and wrap it in the block
            let paragraph = Paragraph::new("Hello, World! Press 'q' to quit.")
                .block(block);

            frame.render_widget(paragraph, size);
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    should_quit = true;
                }
            }
        }
    }

    // Restore terminal state on exit
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

