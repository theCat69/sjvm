use anyhow::Context;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::jdk_switcher::{jdk_display_name, switch_to_jdk};
use crate::memory::memory;
use crate::symlinks::symlink_path;

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
    fn new() -> anyhow::Result<Self> {
        let jdks = &memory().jdks;
        let current_link = symlink_path();
        // If the symlink cannot be read, fall back to an empty path so the TUI
        // still renders — the current indicator simply won't be shown.
        let current = std::fs::read_link(&current_link)
            .with_context(|| format!("Cannot read symlink '{}'", current_link.display()))
            .unwrap_or_else(|_| PathBuf::new());

        let mut items = Vec::new();
        let mut selected_index = None;
        let mut current_jdk = None;

        for (index, jdk) in jdks.iter().enumerate() {
            let is_current = jdk == &current;
            let display_name = jdk_display_name(jdk);

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
        if self.items.is_empty() {
            return;
        }
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
        if self.items.is_empty() {
            return;
        }
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

    fn selected_jdk(&self) -> Option<&JdkItem> {
        self.selected_index.and_then(|i| self.items.get(i))
    }

    fn switch_to_selected(&mut self) -> anyhow::Result<bool> {
        if let Some(jdk_item) = self.selected_jdk() {
            let jdk_path = jdk_item.path.clone();
            let display_name = jdk_item.display_name.clone();
            switch_to_jdk(&jdk_path)?;
            self.current_jdk = Some(jdk_path.clone());

            // Update current status
            for item in &mut self.items {
                item.is_current = item.path == jdk_path;
            }

            // Set success message with timestamp
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

/// Runs the interactive TUI using ratatui's built-in init/restore pattern,
/// which installs a panic hook to ensure the terminal is always cleaned up.
fn run_ui() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let result = run_app_loop(&mut terminal);
    ratatui::restore();
    result
}

fn run_app_loop(terminal: &mut ratatui::DefaultTerminal) -> anyhow::Result<()> {
    let mut app = App::new()?;

    loop {
        // Clear expired success message
        app.clear_expired_success();

        terminal.draw(|f| render_ui(f, &mut app))?;

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

    Ok(())
}

fn render_ui(f: &mut Frame, app: &mut App) {
    // Fixed layout with permanent status bar at top
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Status bar (always visible)
            Constraint::Min(3),    // List
            Constraint::Length(3), // Help
        ])
        .split(f.area());

    // Render status bar (styled like Help section)
    let status_content = if let Some(ref message) = app.success_message {
        Line::from(vec![
            ratatui::text::Span::styled("✓ ", Style::default().fg(Color::Green)),
            ratatui::text::Span::raw(message),
        ])
    } else {
        Line::from("")
    };

    let status = Paragraph::new(status_content).block(
        Block::default()
            .borders(Borders::ALL)
            .title("SJVM - A simple Java version manager"),
    );
    f.render_widget(status, chunks[0]);

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

    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    let help_text = vec![Line::from(
        "↑/k: Up   ↓/j: Down   Enter: Select   q/Esc: Quit",
    )];

    let help =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Help"));
    f.render_widget(help, chunks[2]);
}

/// Launches the interactive JDK selector TUI.
///
/// Renders a full-screen terminal UI that lets the user navigate the list of
/// available JDKs with `j`/`k` (or arrow keys) and press `Enter` to switch.
/// Press `q` or `Esc` to exit without making a change.
///
/// # Errors
/// Returns an error if the terminal cannot be initialised, if reading the
/// current symlink fails, or if the selected JDK cannot be activated.
pub(crate) fn interactive_select() -> anyhow::Result<()> {
    run_ui()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
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
        assert_eq!(app.selected_jdk().unwrap().display_name, "jdk-11");

        // Test next navigation
        app.next();
        assert_eq!(app.selected_index, Some(1));
        assert_eq!(app.selected_jdk().unwrap().display_name, "jdk-17");

        // Test previous navigation
        app.previous();
        assert_eq!(app.selected_index, Some(0));
        assert_eq!(app.selected_jdk().unwrap().display_name, "jdk-11");

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
        assert_eq!(app.selected_jdk().unwrap().display_name, "jdk-21");
    }

    #[test]
    fn test_ui_rendering() {
        let mut app = create_test_app();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        // This should not panic and should render UI successfully
        let result = terminal.draw(|f| render_ui(f, &mut app));
        assert!(result.is_ok(), "UI rendering should not fail");
    }

    #[test]
    fn test_ui_help_text_rendering() {
        let mut app = create_test_app();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| render_ui(f, &mut app)).unwrap();

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
        let mut app = create_test_app();
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        // Just verify that rendering doesn't crash
        let result = terminal.draw(|f| render_ui(f, &mut app));
        assert!(result.is_ok(), "List item rendering should not fail");
    }

    #[test]
    fn test_current_jdk_indicator() {
        let mut app = create_test_app();
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| render_ui(f, &mut app)).unwrap();

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
    fn test_navigation_on_empty_list() {
        let mut app = App {
            items: vec![],
            list_state: ListState::default(),
            selected_index: None,
            current_jdk: None,
            success_message: None,
            success_shown_at: None,
        };
        // Neither next() nor previous() should panic on an empty list
        app.next();
        assert_eq!(app.selected_index, None);
        app.previous();
        assert_eq!(app.selected_index, None);
    }

    #[test]
    fn test_app_selection_logic() {
        let mut app = create_test_app();

        // Test getting selected JDK
        let selected = app.selected_jdk();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().display_name, "jdk-11");

        // Test selection after navigation
        app.next();
        let selected = app.selected_jdk();
        assert_eq!(selected.unwrap().display_name, "jdk-17");

        app.next();
        let selected = app.selected_jdk();
        assert_eq!(selected.unwrap().display_name, "jdk-21");
    }
}
