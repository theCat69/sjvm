<!-- Demonstrates feature-gated ratatui render loop from src/commands/ui/mod.rs -->

```rust
// src/commands/ui/mod.rs (excerpt)
fn render_ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(3),    // screen content
        ])
        .split(f.area());

    render_tab_bar(f, chunks[0], &app.screen);

    match app.screen {
        Screen::Switch => {
            crate::commands::ui::switch_screen::render_switch_screen(f, &mut app.switch, chunks[1]);
        }
        Screen::Install => {
            crate::commands::ui::install_screen::render_install_screen(
                f,
                &mut app.install,
                chunks[1],
                app.install_vendor.as_ref(),
            );
        }
    }
}

fn run_app_loop(terminal: &mut ratatui::DefaultTerminal) -> anyhow::Result<()> {
    let mut app = App::new()?;
    loop {
        terminal.draw(|f| render_ui(f, &mut app))?;
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            // handle key events and update app state
        }
    }
}
```
