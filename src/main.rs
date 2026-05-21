mod app;
mod event;
mod http;
mod input;
mod models;
mod postman;
mod storage;
mod theme;
mod ui;

use std::{io, panic};

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{app::App, event::run_app};

#[tokio::main]
async fn main() -> Result<()> {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        hook(info);
    }));

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = async {
        let mut app = App::new()?;
        run_app(&mut terminal, &mut app).await
    }
    .await;

    restore_terminal()?;
    result
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}
