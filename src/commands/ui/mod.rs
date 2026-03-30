pub(crate) mod install_screen;
pub(crate) mod switch_screen;

use std::sync::mpsc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Tabs},
};

use crate::commands::ui::install_screen::{
    CatalogEvent, DownloadEvent, InstallState, VersionsEvent,
};
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
    versions_rx: Option<mpsc::Receiver<VersionsEvent>>,
}

impl App {
    fn new() -> anyhow::Result<Self> {
        Ok(App {
            screen: Screen::Switch,
            switch: SwitchState::new()?,
            install: InstallState::VendorPicker { selected: 0 },
            download_rx: None,
            catalog_rx: None,
            versions_rx: None,
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

        // Drain download channel (all available events; keep last Progress)
        if let Some(ref rx) = app.download_rx {
            loop {
                match rx.try_recv() {
                    Ok(DownloadEvent::Progress { downloaded, total }) => {
                        app.install = InstallState::Downloading { downloaded, total };
                    }
                    Ok(DownloadEvent::Done { jdk_dir }) => {
                        app.install = InstallState::Installed { jdk_path: jdk_dir };
                        app.download_rx = None;
                        break;
                    }
                    Ok(DownloadEvent::Error { message }) => {
                        app.install = InstallState::Failed { message };
                        app.download_rx = None;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        app.download_rx = None;
                        break;
                    }
                }
            }
        }

        // Drain catalog channel (single result event)
        if let Some(ref rx) = app.catalog_rx {
            match rx.try_recv() {
                Ok(CatalogEvent::Resolved(artifact)) => {
                    app.install = InstallState::ArtifactReady { artifact };
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

        // Drain versions channel (single result event)
        if let Some(ref rx) = app.versions_rx {
            match rx.try_recv() {
                Ok(VersionsEvent::Fetched(versions)) => {
                    // Extract vendor from current state
                    if let InstallState::FetchingVersions { vendor } = &app.install {
                        let vendor = vendor.clone();
                        app.install = InstallState::VersionPicker {
                            vendor,
                            versions,
                            selected: 0,
                        };
                    }
                    app.versions_rx = None;
                }
                Ok(VersionsEvent::Error(msg)) => {
                    app.install = InstallState::Failed { message: msg };
                    app.versions_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    app.versions_rx = None;
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

            // Global Ctrl+C / q / Esc quit — unless delete overlay is active
            if app.switch.delete_confirm.is_none()
                && (key.code == KeyCode::Char('q') || key.code == KeyCode::Esc)
            {
                break;
            }

            match key.code {
                KeyCode::Tab => {
                    app.screen = match app.screen {
                        Screen::Switch => Screen::Install,
                        Screen::Install => Screen::Switch,
                    };
                }
                KeyCode::Char('s') => app.screen = Screen::Switch,
                KeyCode::Char('i') => app.screen = Screen::Install,
                _ => match app.screen {
                    Screen::Switch => handle_switch_key(&mut app, key)?,
                    Screen::Install => {
                        handle_install_key(&mut app, key)?;
                    }
                },
            }
        }
    }
    Ok(())
}

fn handle_switch_key(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    use crate::commands::delete::delete_jdk;

    // If delete overlay is active, intercept all keys
    if app.switch.delete_confirm.is_some() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(name) = app.switch.delete_confirm.take() {
                    match delete_jdk(&name) {
                        Ok(_) => {
                            app.switch = SwitchState::new().unwrap_or_else(|_| SwitchState {
                                items: vec![],
                                list_state: ratatui::widgets::ListState::default(),
                                selected_index: None,
                                current_jdk: None,
                                success_message: Some("Deleted — please reload".to_owned()),
                                success_shown_at: Some(std::time::Instant::now()),
                                delete_confirm: None,
                            });
                        }
                        Err(e) => {
                            app.switch.success_message = Some(format!("Delete failed: {e}"));
                            app.switch.success_shown_at = Some(std::time::Instant::now());
                        }
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                app.switch.delete_confirm = None;
            }
            _ if key.modifiers.contains(KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('c') =>
            {
                app.switch.delete_confirm = None;
            }
            _ => {}
        }
        return Ok(());
    }

    // Normal switch key handling
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.switch.previous(),
        KeyCode::Down | KeyCode::Char('j') => app.switch.next(),
        KeyCode::Enter => {
            app.switch.switch_to_selected()?;
        }
        KeyCode::Char('d') => {
            if let Some(item) = app.switch.selected_jdk() {
                app.switch.delete_confirm = Some(item.display_name.clone());
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_install_key(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    use crate::commands::ui::install_screen::{
        spawn_catalog_fetch, spawn_download, spawn_versions_fetch,
    };
    use crate::infra::config::config;

    let ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');

    match &app.install {
        InstallState::VendorPicker { selected } => {
            let selected = *selected;
            match key.code {
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
                    let (tx, rx) = mpsc::channel::<VersionsEvent>();
                    app.versions_rx = Some(rx);
                    spawn_versions_fetch(vendor.clone(), tx);
                    app.install = InstallState::FetchingVersions { vendor };
                }
                _ if ctrl_c => {
                    app.screen = Screen::Switch;
                }
                _ => {}
            }
        }
        InstallState::FetchingVersions { .. } => {
            if ctrl_c {
                app.install = InstallState::VendorPicker { selected: 0 };
                app.versions_rx = None;
            }
        }
        InstallState::VersionPicker {
            vendor,
            versions,
            selected,
        } => {
            let vendor = vendor.clone();
            let versions = versions.clone();
            let selected = *selected;

            if ctrl_c {
                app.install = InstallState::VendorPicker { selected: 0 };
            } else {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        let new_sel =
                            (selected + versions.len().saturating_sub(1)) % versions.len().max(1);
                        app.install = InstallState::VersionPicker {
                            vendor,
                            versions,
                            selected: new_sel,
                        };
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let new_sel = if versions.is_empty() {
                            0
                        } else {
                            (selected + 1) % versions.len()
                        };
                        app.install = InstallState::VersionPicker {
                            vendor,
                            versions,
                            selected: new_sel,
                        };
                    }
                    KeyCode::Enter => {
                        if !versions.is_empty() {
                            let version = versions[selected];
                            let (tx, rx) = mpsc::channel::<CatalogEvent>();
                            app.catalog_rx = Some(rx);
                            spawn_catalog_fetch(vendor.clone(), version, tx);
                            app.install = InstallState::FetchingArtifact { vendor, version };
                        }
                    }
                    _ => {}
                }
            }
        }
        InstallState::FetchingArtifact { .. } => {
            if ctrl_c {
                app.install = InstallState::VendorPicker { selected: 0 };
                app.catalog_rx = None;
            }
        }
        InstallState::ArtifactReady { artifact } => {
            let artifact = artifact.clone();
            if ctrl_c {
                app.install = InstallState::VendorPicker { selected: 0 };
            } else if key.code == KeyCode::Enter {
                let dest_dir = match config().jdks_dirs.first() {
                    Some(d) => std::path::PathBuf::from(d),
                    None => {
                        app.install = InstallState::Failed {
                            message: "No JDKs directory configured".to_owned(),
                        };
                        return Ok(());
                    }
                };
                app.install = InstallState::Downloading {
                    downloaded: 0,
                    total: None,
                };
                let (tx, rx) = mpsc::channel::<DownloadEvent>();
                app.download_rx = Some(rx);
                spawn_download(artifact, dest_dir, tx);
            }
        }
        InstallState::Downloading { .. } => {
            // Cannot cancel — background thread is running
        }
        InstallState::Installed { jdk_path } => {
            let jdk_path = jdk_path.clone();
            if key.code == KeyCode::Char('y') {
                crate::core::jdk_switcher::switch_to_jdk(&jdk_path)?;
                app.switch = SwitchState::new()?;
                app.install = InstallState::VendorPicker { selected: 0 };
                app.screen = Screen::Switch;
            } else if ctrl_c || key.code == KeyCode::Char('n') {
                app.install = InstallState::VendorPicker { selected: 0 };
            }
        }
        InstallState::Failed { .. } => {
            if ctrl_c {
                app.install = InstallState::VendorPicker { selected: 0 };
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
