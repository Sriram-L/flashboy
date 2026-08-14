use flashboy::app::App;
use flashboy::ui;

use std::io::{self, stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;


fn main() {
    if let Err(e) = run() {
        let _ = restore();
        eprintln!("{e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let arg = std::env::args().nth(1).map(PathBuf::from);
    if matches!(arg.as_ref().and_then(|p| p.to_str()), Some("-h" | "--help")) {
        print_help();
        return Ok(());
    }

    let mut app = App::new(arg)?;
    setup()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let result = loop_ui(&mut terminal, &mut app);
    restore()?;
    result
}

fn loop_ui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    while !app.should_quit {
        app.drain_worker();
        terminal.draw(|f| ui::draw(f, app))?;
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if key.kind == event::KeyEventKind::Press => app.on_key(key),
                Event::Mouse(mouse) => app.on_mouse(mouse),
                _ => {}
            }
        }
    }
    Ok(())
}

fn setup() -> Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    std::panic::set_hook(Box::new(|info| {
        let _ = restore();
        eprintln!("{info}");
    }));
    Ok(())
}

fn restore() -> Result<()> {
    let _ = disable_raw_mode();
    execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

fn print_help() {
    println!(
        "\
flashboy — TUI test runner for C++ programs

Usage:
  flashboy [path/to/program.cpp]

Keys:
  n / e / d     create, edit, delete a case
  r / R         run selected / run all
  q             quit (saves)

Storage:
  one compact file beside the source: program.cpp.fbk
  format: FBK1 magic + zstd(postcard)

Judge:
  compile with Homebrew g++-12 (-std=c++17 -O2)
  5s wall-clock TLE per case
  trailing whitespace is ignored when comparing

Env:
  FLASHBOY_CXX   override compiler (default: /opt/homebrew/bin/g++-12)"
    );
}
