pub(crate) mod install_screen;
pub(crate) mod switch_screen;

use std::sync::mpsc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Tabs},
};

use crate::commands::ui::install_screen::{CatalogEvent, DownloadEvent, InstallState};
use crate::commands::ui::switch_screen::SwitchState;

#[derive(Debug, Clone, PartialEq)]
enum Screen {
    Switch,
    Install,
}

struct App {
    screen: Screen,
    switch: SwitchState,
    install: InstallState,
    download_rx: Option<mpsc::Receiver<DownloadEvent>>,
    catalog_rx: Option<mpsc::Receiver<CatalogEvent>>,
}

impl App {
    fn new() -> anyhow::Result<Self> {
        Ok(App {
            screen: Screen::Switch,
            switch: SwitchState::new()?,
            install: InstallState::Idle,
            download_rx: None,
            catalog_rx: None,
        })
    }
}

fn render_tab_bar(f: &mut Frame, area: ratatui::layout::Rect, active: &Screen) {
    let titles = vec!["[S]witch", "[I]nstall"];
    let selected = match active {
        Screen::Switch => 0,
        Screen::Install => 1,
    };
    let tabs = Tabs::new(titles)
        .select(selected)
        .block(Block::default().borders(Borders::ALL).title("sjvm"))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider("|");
    f.render_widget(tabs, area);
}

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
            );
        }
    }
}

fn run_app_loop(terminal: &mut ratatui::DefaultTerminal) -> anyhow::Result<()> {
    let mut app = App::new()?;

    loop {
        app.switch.clear_expired_success();

        // Drain download channel
        if let Some(ref rx) = app.download_rx {
            match rx.try_recv() {
                Ok(DownloadEvent::Progress { downloaded, total }) => {
                    app.install = InstallState::Downloading {
                        progress: if total > 0 {
                            downloaded as f64 / total as f64
                        } else {
                            0.0
                        },
                        label: format!("{}/{} bytes", downloaded, total),
                    };
                }
                Ok(DownloadEvent::Done { jdk_dir }) => {
                    app.install = InstallState::Installed { jdk_path: jdk_dir };
                    app.download_rx = None;
                }
                Ok(DownloadEvent::Error { message }) => {
                    app.install = InstallState::Failed { message };
                    app.download_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    app.download_rx = None;
                }
            }
        }

        // Drain catalog channel
        if let Some(ref rx) = app.catalog_rx {
            match rx.try_recv() {
                Ok(CatalogEvent::Resolved(artifact)) => {
                    app.install = InstallState::VersionList { artifact };
                    app.catalog_rx = None;
                }
                Ok(CatalogEvent::Error(message)) => {
                    app.install = InstallState::Failed { message };
                    app.catalog_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    app.catalog_rx = None;
                }
            }
        }

        terminal.draw(|f| render_ui(f, &mut app))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Tab => {
                    app.screen = match app.screen {
                        Screen::Switch => Screen::Install,
                        Screen::Install => Screen::Switch,
                    };
                }
                KeyCode::Char('s') => app.screen = Screen::Switch,
                KeyCode::Char('i') => app.screen = Screen::Install,
                _ => match app.screen {
                    Screen::Switch => match key.code {
                        KeyCode::Up | KeyCode::Char('k') => app.switch.previous(),
                        KeyCode::Down | KeyCode::Char('j') => app.switch.next(),
                        KeyCode::Enter => {
                            app.switch.switch_to_selected()?;
                        }
                        _ => {}
                    },
                    Screen::Install => {
                        handle_install_key(&mut app, key.code)?;
                    }
                },
            }
        }
    }
    Ok(())
}

fn handle_install_key(app: &mut App, key: KeyCode) -> anyhow::Result<()> {
    use crate::commands::ui::install_screen::{spawn_catalog_fetch, spawn_download};
    use crate::infra::config::config;

    match &app.install {
        InstallState::Idle => {
            if key == KeyCode::Enter {
                app.install = InstallState::VendorPicker { selected: 0 };
            }
        }
        InstallState::VendorPicker { selected } => {
            let selected = *selected;
            match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    let new_sel = if selected == 0 { 1 } else { selected - 1 };
                    app.install = InstallState::VendorPicker { selected: new_sel };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    app.install = InstallState::VendorPicker {
                        selected: (selected + 1) % 2,
                    };
                }
                KeyCode::Enter => {
                    let vendor = if selected == 0 {
                        crate::core::jdk_catalog::Vendor::OpenJdk
                    } else {
                        crate::core::jdk_catalog::Vendor::GraalVm
                    };
                    app.install = InstallState::FetchingVersions;
                    let (tx, rx) = mpsc::channel::<CatalogEvent>();
                    app.catalog_rx = Some(rx);
                    spawn_catalog_fetch(vendor, tx);
                }
                KeyCode::Esc => {
                    app.install = InstallState::Idle;
                }
                _ => {}
            }
        }
        InstallState::VersionList { artifact } => {
            let artifact = artifact.clone();
            if key == KeyCode::Enter {
                let dest_dir = match config().jdks_dirs.first().cloned() {
                    Some(d) => std::path::PathBuf::from(d),
                    None => {
                        app.install = InstallState::Failed {
                            message: "No JDKs directory configured".to_owned(),
                        };
                        return Ok(());
                    }
                };
                app.install = InstallState::Downloading {
                    progress: 0.0,
                    label: "Starting…".to_owned(),
                };
                let (tx, rx) = mpsc::channel::<DownloadEvent>();
                app.download_rx = Some(rx);
                spawn_download(artifact, dest_dir, tx);
            } else if key == KeyCode::Esc {
                app.install = InstallState::Idle;
            }
        }
        InstallState::Installed { jdk_path } => {
            let jdk_path = jdk_path.clone();
            if key == KeyCode::Char('y') {
                crate::core::jdk_switcher::switch_to_jdk(&jdk_path)?;
                app.switch = SwitchState::new()?;
                app.install = InstallState::Idle;
                app.screen = Screen::Switch;
            } else if key == KeyCode::Esc || key == KeyCode::Char('n') {
                app.install = InstallState::Idle;
            }
        }
        InstallState::Failed { .. }
        | InstallState::FetchingVersions
        | InstallState::Downloading { .. } => {
            if key == KeyCode::Esc {
                app.install = InstallState::Idle;
            }
        }
    }
    Ok(())
}

/// Runs the interactive TUI using ratatui's built-in init/restore pattern,
/// which installs a panic hook to ensure the terminal is always cleaned up.
fn run_ui() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let result = run_app_loop(&mut terminal);
    ratatui::restore();
    result
}

/// Launches the interactive JDK selector TUI.
///
/// Renders a full-screen terminal UI that lets the user switch JDKs (Switch tab)
/// or install a new JDK (Install tab). Press `Tab` to switch screens, `q` or
/// `Esc` to exit.
///
/// # Errors
/// Returns an error if the terminal cannot be initialised, if reading the
/// current symlink fails, or if the selected JDK cannot be activated.
pub(crate) fn interactive_select() -> anyhow::Result<()> {
    run_ui()?;
    Ok(())
}
