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
use std::time::{Duration, Instant};

use crate::memory::memory;
use crate::symlinks::{create_symlink, get_symlink_path};

struct App {
    items: Vec<JdkItem>,
    list_state: ListState,
    selected_index: Option<usize>,
    current_jdk: Option<PathBuf>,
    success_message: Option<String>,
    success_shown_at: Option<Instant>,
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
            success_message: None,
            success_shown_at: None,
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
            let display_name = jdk_item.display_name.clone();
            let symlink = get_symlink_path();
            create_symlink(&jdk_path, &symlink)?;
            self.current_jdk = Some(jdk_path.clone());

            // Update current status
            for item in &mut self.items {
                item.is_current = item.path == jdk_path;
            }

            // Set success message
            self.success_message = Some(format!("Switched to {}", display_name));
            self.success_shown_at = Some(Instant::now());

            return Ok(true);
        }
        Ok(false)
    }

    fn clear_expired_success(&mut self) {
        if let Some(shown_at) = self.success_shown_at
            && shown_at.elapsed() > Duration::from_secs(2)
        {
            self.success_message = None;
            self.success_shown_at = None;
        }
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
        // Clear expired success message
        app.clear_expired_success();

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
                    app.switch_to_selected()?;
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
    // Build layout based on whether we have a success message
    let chunks = if app.success_message.is_some() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Status bar
                Constraint::Min(3),    // List
                Constraint::Length(3), // Help
            ])
            .split(f.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // List
                Constraint::Length(3), // Help
            ])
            .split(f.area())
    };

    // Render status bar if there's a success message
    let (list_chunk, help_chunk) = if let Some(ref message) = app.success_message {
        let status_line = Line::from(vec![
            ratatui::text::Span::styled("✓ ", Style::default().fg(Color::Green)),
            ratatui::text::Span::raw(message),
        ]);
        let status = Paragraph::new(status_line);
        f.render_widget(status, chunks[0]);
        (chunks[1], chunks[2])
    } else {
        (chunks[0], chunks[1])
    };

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

    f.render_stateful_widget(list, list_chunk, &mut app.list_state.clone());

    let help_text = vec![Line::from(
        "↑/k: Up   ↓/j: Down   Enter: Select   q/Esc: Quit",
    )];

    let help =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Help"));
    f.render_widget(help, help_chunk);
}

pub fn interactive_select() {
    if let Err(e) = run_ui() {
        eprintln!("❌ Error running interactive UI: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    fn create_test_app() -> App {
        // Create a mock app for testing
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        App {
            items: vec![
                JdkItem {
                    path: PathBuf::from("/test/jdk-11"),
                    display_name: "jdk-11".to_string(),
                    is_current: true,
                },
                JdkItem {
                    path: PathBuf::from("/test/jdk-17"),
                    display_name: "jdk-17".to_string(),
                    is_current: false,
                },
                JdkItem {
                    path: PathBuf::from("/test/jdk-21"),
                    display_name: "jdk-21".to_string(),
                    is_current: false,
                },
            ],
            list_state,
            selected_index: Some(0),
            current_jdk: Some(PathBuf::from("/test/jdk-11")),
            success_message: None,
            success_shown_at: None,
        }
    }

    #[test]
    fn test_app_navigation() {
        let mut app = create_test_app();

        // Test initial selection
        assert_eq!(app.selected_index, Some(0));
        assert_eq!(app.get_selected_jdk().unwrap().display_name, "jdk-11");

        // Test next navigation
        app.next();
        assert_eq!(app.selected_index, Some(1));
        assert_eq!(app.get_selected_jdk().unwrap().display_name, "jdk-17");

        // Test previous navigation
        app.previous();
        assert_eq!(app.selected_index, Some(0));
        assert_eq!(app.get_selected_jdk().unwrap().display_name, "jdk-11");

        // Test wrap around on next
        app.next(); // to index 1
        app.next(); // to index 2
        app.next(); // should wrap to index 0
        assert_eq!(app.selected_index, Some(0));
    }

    #[test]
    fn test_app_wrap_navigation() {
        let mut app = create_test_app();

        // Test wrap around on previous from first item
        app.previous();
        assert_eq!(app.selected_index, Some(2)); // Should wrap to last item
        assert_eq!(app.get_selected_jdk().unwrap().display_name, "jdk-21");
    }

    #[test]
    fn test_ui_rendering() {
        let app = create_test_app();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        // This should not panic and should render UI successfully
        let result = terminal.draw(|f| ui(f, &app));
        assert!(result.is_ok(), "UI rendering should not fail");
    }

    #[test]
    fn test_ui_help_text_rendering() {
        let app = create_test_app();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| ui(f, &app)).unwrap();

        // Check that help section is rendered by looking at buffer
        let buffer = terminal.backend().buffer();
        let content = buffer.content();

        // Look for help text characters
        let help_found = content.iter().any(|cell: &ratatui::buffer::Cell| {
            let symbol = cell.symbol();
            symbol.contains("Up")
                || symbol.contains("Down")
                || symbol.contains("Enter")
                || symbol.contains("q")
        });
        assert!(
            help_found,
            "Help text with navigation instructions should be rendered"
        );
    }

    #[test]
    fn test_list_item_rendering() {
        let app = create_test_app();
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        // Just verify that rendering doesn't crash
        let result = terminal.draw(|f| ui(f, &app));
        assert!(result.is_ok(), "List item rendering should not fail");
    }

    #[test]
    fn test_current_jdk_indicator() {
        let app = create_test_app();
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| ui(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer.content();

        // Verify that current indicator (→) is rendered for jdk-11
        let current_indicator_found = content
            .iter()
            .any(|cell: &ratatui::buffer::Cell| cell.symbol().contains("→"));
        assert!(
            current_indicator_found,
            "Current JDK indicator (→) should be rendered"
        );
    }

    #[test]
    fn test_app_selection_logic() {
        let mut app = create_test_app();

        // Test getting selected JDK
        let selected = app.get_selected_jdk();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().display_name, "jdk-11");

        // Test selection after navigation
        app.next();
        let selected = app.get_selected_jdk();
        assert_eq!(selected.unwrap().display_name, "jdk-17");

        app.next();
        let selected = app.get_selected_jdk();
        assert_eq!(selected.unwrap().display_name, "jdk-21");
    }
}
