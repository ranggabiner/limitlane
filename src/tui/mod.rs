pub mod app;
pub mod events;
pub mod ui;
pub mod widgets;

use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::error::LimitLaneError;
use crate::storage::DatabaseRepository;

pub use app::{ApplicationState, Screen};
pub use events::{Event, EventHandler};

pub struct TerminalGuard;

impl TerminalGuard {
    pub fn init() -> Result<Self, LimitLaneError> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, Hide)?;
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, Show);
    }
}

pub async fn run_tui(db: Arc<DatabaseRepository>) -> Result<(), LimitLaneError> {
    let _guard = TerminalGuard::init()?;

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let mut stdout = stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, Show);
        original_hook(panic_info);
    }));

    let mut stdout = stdout();
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let accounts = db.list_accounts()?;
    let mut snapshots = Vec::new();
    for acc in &accounts {
        if let Some(snap) = db.get_latest_snapshot(&acc.id)? {
            snapshots.push(snap);
        }
    }

    let mut app = ApplicationState::new(accounts, snapshots);
    let mut events = EventHandler::new(Duration::from_millis(250));

    loop {
        terminal.draw(|f| ui::render(f, &app))?;

        if app.should_quit {
            break;
        }

        if let Some(event) = events.next().await {
            match event {
                Event::Key(key) => {
                    app.handle_key_event(key);
                }
                Event::Resize(_, _) => {
                    terminal.autoresize()?;
                }
                Event::Tick => {}
                Event::RefreshRequested(acc_id) => {
                    if let Some(_id) = acc_id {
                        app.status_message = Some("Refreshing account usage...".to_string());
                    } else {
                        app.status_message = Some("Refreshing all usage data...".to_string());
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}
