use anyhow::Context;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crate::memory::memory;
use crate::symlinks::{create_symlink, get_symlink_path};

struct App {
    items: Vec<JdkItem>,
    list_state: ListState,
    selected_index: Option<usize>,
    current_jdk: Option<PathBuf>,
}

#[derive(Clone)]
struct JdkItem {
    path: PathBuf,
    display_name: String,
    is_current: bool,
}

impl App {
    fn new() -> Result<Self, anyhow::Error> {
        let jdks = &memory().jdks;
        let current_link = get_symlink_path();
        let current = std::fs::read_link(&current_link)
            .with_context(|| "Cannot read current link")
            .unwrap_or_default();

        let mut items = Vec::new();
        let mut selected_index = None;
        let mut current_jdk = None;

        for (index, jdk) in jdks.iter().enumerate() {
            let is_current = jdk == &current;
            let display_name = jdk
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            if is_current {
                selected_index = Some(index);
                current_jdk = Some(jdk.clone());
            }

            items.push(JdkItem {
                path: jdk.clone(),
                display_name,
                is_current,
            });
        }

        let mut list_state = ListState::default();
        list_state.select(selected_index);

        Ok(App {
            items,
            list_state,
            selected_index,
            current_jdk,
        })
    }

    fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.selected_index = Some(i);
    }

    fn previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.selected_index = Some(i);
    }

    fn get_selected_jdk(&self) -> Option<&JdkItem> {
        self.selected_index.and_then(|i| self.items.get(i))
    }

    fn switch_to_selected(&mut self) -> Result<bool, anyhow::Error> {
        if let Some(jdk_item) = self.get_selected_jdk() {
            let jdk_path = jdk_item.path.clone();
            let symlink = get_symlink_path();
            create_symlink(&jdk_path, &symlink)?;
            self.current_jdk = Some(jdk_path.clone());

            // Update current status
            for item in &mut self.items {
                item.is_current = item.path == jdk_path;
            }

            return Ok(true);
        }
        Ok(false)
    }
}

fn run_ui() -> Result<(), anyhow::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Up | KeyCode::Char('k') => app.previous(),
                KeyCode::Down | KeyCode::Char('j') => app.next(),
                KeyCode::Enter => {
                    if app.switch_to_selected()? {
                        // Success - show brief message and exit
                        terminal.draw(|f| {
                            let area = f.area();
                            let msg = Paragraph::new("✅ Successfully switched to selected JDK")
                                .style(Style::default().fg(Color::Green))
                                .block(Block::default().borders(Borders::ALL));
                            f.render_widget(msg, area);
                        })?;

                        std::thread::sleep(Duration::from_secs(1));
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
        .split(f.area());

    let items: Vec<ListItem> = app
        .items
        .iter()
        .map(|item| {
            let prefix = if item.is_current { "→ " } else { "  " };
            let line = Line::from(format!("{} {}", prefix, item.display_name));
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Select JDK Version"),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, chunks[0], &mut app.list_state.clone());

    let help_text = vec![Line::from(
        "↑/k: Up   ↓/j: Down   Enter: Select   q/Esc: Quit",
    )];

    let help =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Help"));
    f.render_widget(help, chunks[1]);
}

pub fn interactive_select() {
    if let Err(e) = run_ui() {
        eprintln!("❌ Error running interactive UI: {}", e);
        std::process::exit(1);
    }
}
