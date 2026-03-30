use std::path::PathBuf;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph},
};

use crate::core::jdk_catalog::ArtifactInfo;

/// Represents the current state of the install screen flow.
#[derive(Clone)]
pub(crate) enum InstallState {
    /// No action in progress; shows prompt to start install.
    Idle,
    /// User is selecting a JDK vendor. `selected` is 0=OpenJDK, 1=GraalVM.
    VendorPicker { selected: usize },
    /// API fetch is in progress.
    FetchingVersions,
    /// API resolved an artifact and it is ready to download.
    VersionList { artifact: ArtifactInfo },
    /// Download is in progress.
    Downloading { progress: f64, label: String },
    /// JDK was successfully installed at `jdk_path`.
    Installed { jdk_path: PathBuf },
    /// An error occurred.
    Failed { message: String },
}

/// Events emitted by the background download thread.
pub(crate) enum DownloadEvent {
    Progress { downloaded: u64, total: u64 },
    Done { jdk_dir: PathBuf },
    Error { message: String },
}

/// Events emitted by the background catalog-fetch thread.
pub(crate) enum CatalogEvent {
    Resolved(ArtifactInfo),
    Error(String),
}

/// Renders the install screen into `area`, delegating to the appropriate sub-renderer
/// based on the current `state`.
pub(crate) fn render_install_screen(
    f: &mut Frame,
    state: &mut InstallState,
    area: ratatui::layout::Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // vendor picker / status header
            Constraint::Min(3),    // main content
            Constraint::Length(3), // progress gauge
            Constraint::Length(3), // help
        ])
        .split(area);

    match state {
        InstallState::Idle => {
            render_header(f, chunks[0], "Install JDK");
            let content = Paragraph::new(Line::from("Press Enter to install a JDK"))
                .block(Block::default().borders(Borders::ALL).title("Install"));
            f.render_widget(content, chunks[1]);
            render_empty_gauge(f, chunks[2]);
            render_help(f, chunks[3], "Enter: Start install   q: Quit");
        }
        InstallState::VendorPicker { selected } => {
            let selected = *selected;
            render_header(f, chunks[0], "Select Vendor");
            render_vendor_list(f, chunks[1], selected);
            render_empty_gauge(f, chunks[2]);
            render_help(f, chunks[3], "↑/↓: Select   Enter: Choose   Esc: Cancel");
        }
        InstallState::FetchingVersions => {
            render_header(f, chunks[0], "Fetching Versions");
            let content = Paragraph::new(Line::from("Fetching available versions..."))
                .block(Block::default().borders(Borders::ALL).title("Status"));
            f.render_widget(content, chunks[1]);
            render_empty_gauge(f, chunks[2]);
            render_help(f, chunks[3], "Please wait...");
        }
        InstallState::VersionList { artifact } => {
            render_header(f, chunks[0], "Version Ready");
            let text = vec![
                Line::from(format!("File:  {}", artifact.filename)),
                Line::from(format!("URL:   {}", artifact.download_url)),
            ];
            let content = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title("Artifact"));
            f.render_widget(content, chunks[1]);
            render_empty_gauge(f, chunks[2]);
            render_help(f, chunks[3], "Enter: Download   Esc: Back");
        }
        InstallState::Downloading { progress, label } => {
            let progress = *progress;
            let label = label.clone();
            render_header(f, chunks[0], "Downloading");
            let content = Paragraph::new(Line::from("Downloading..."))
                .block(Block::default().borders(Borders::ALL).title("Progress"));
            f.render_widget(content, chunks[1]);
            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::ALL).title("Progress"))
                .gauge_style(Style::default().fg(Color::Cyan))
                .ratio(progress);
            f.render_widget(gauge, chunks[2]);
            render_help(f, chunks[3], &label);
        }
        InstallState::Installed { jdk_path } => {
            let path_str = jdk_path.display().to_string();
            render_header(f, chunks[0], "Installed");
            let content = Paragraph::new(Line::from(vec![
                ratatui::text::Span::styled("✅ Installed: ", Style::default().fg(Color::Green)),
                ratatui::text::Span::raw(path_str),
            ]))
            .block(Block::default().borders(Borders::ALL).title("Done"));
            f.render_widget(content, chunks[1]);
            render_empty_gauge(f, chunks[2]);
            render_help(f, chunks[3], "y: Switch to this JDK   n/Esc: Return");
        }
        InstallState::Failed { message } => {
            let message = message.clone();
            render_header(f, chunks[0], "Error");
            let content = Paragraph::new(Line::from(vec![
                ratatui::text::Span::styled("❌ Error: ", Style::default().fg(Color::Red)),
                ratatui::text::Span::raw(message),
            ]))
            .block(Block::default().borders(Borders::ALL).title("Error"));
            f.render_widget(content, chunks[1]);
            render_empty_gauge(f, chunks[2]);
            render_help(f, chunks[3], "Esc: Return");
        }
    }
}

// ---------------------------------------------------------------------------
// Private render helpers
// ---------------------------------------------------------------------------

fn render_header(f: &mut Frame, area: ratatui::layout::Rect, title: &str) {
    let header = Paragraph::new(Line::from(title.to_owned()))
        .block(Block::default().borders(Borders::ALL).title("Install JDK"));
    f.render_widget(header, area);
}

fn render_vendor_list(f: &mut Frame, area: ratatui::layout::Rect, selected: usize) {
    let vendors = ["OpenJDK (Adoptium)", "GraalVM CE"];
    let items: Vec<ListItem> = vendors
        .iter()
        .map(|v| ListItem::new(Line::from(*v)))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Vendor"))
        .highlight_style(Style::default().fg(Color::Cyan))
        .highlight_symbol(">> ");

    let mut list_state = ListState::default();
    list_state.select(Some(selected));
    f.render_stateful_widget(list, area, &mut list_state);
}

fn render_empty_gauge(f: &mut Frame, area: ratatui::layout::Rect) {
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Progress"))
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(0.0);
    f.render_widget(gauge, area);
}

fn render_help(f: &mut Frame, area: ratatui::layout::Rect, text: &str) {
    let help = Paragraph::new(Line::from(text.to_owned()))
        .block(Block::default().borders(Borders::ALL).title("Help"));
    f.render_widget(help, area);
}

// ---------------------------------------------------------------------------
// Background thread spawners
// ---------------------------------------------------------------------------

/// Spawns a background thread to download and install a JDK.
///
/// Progress events are sent via `tx` until the operation completes or fails.
pub(crate) fn spawn_download(
    artifact: ArtifactInfo,
    dest_dir: PathBuf,
    tx: std::sync::mpsc::Sender<DownloadEvent>,
) {
    use crate::core::downloader::{InstallRequest, install_jdk};

    std::thread::spawn(move || {
        let request = InstallRequest {
            artifact,
            dest_dir,
            force: false,
        };
        let tx_progress = tx.clone();
        let result = install_jdk(request, move |downloaded, total| {
            let _ = tx_progress.send(DownloadEvent::Progress {
                downloaded,
                total: total.unwrap_or(0),
            });
        });
        match result {
            Ok(jdk_dir) => {
                let _ = tx.send(DownloadEvent::Done { jdk_dir });
            }
            Err(e) => {
                let _ = tx.send(DownloadEvent::Error {
                    message: format!("{e:#}"),
                });
            }
        }
    });
}

/// Spawns a background thread to fetch artifact info from the vendor API.
///
/// Resolves against JDK 21 (latest LTS). The result is sent via `tx`.
pub(crate) fn spawn_catalog_fetch(
    vendor: crate::core::jdk_catalog::Vendor,
    tx: std::sync::mpsc::Sender<CatalogEvent>,
) {
    use crate::core::jdk_catalog::{detect_arch, detect_os, resolve_artifact};

    std::thread::spawn(move || {
        let result = (|| -> anyhow::Result<ArtifactInfo> {
            let os = detect_os()?;
            let arch = detect_arch()?;
            // Use the latest LTS (21) as the default version for TUI catalog fetch.
            resolve_artifact(&vendor, 21, &os, &arch)
        })();
        match result {
            Ok(artifact) => {
                let _ = tx.send(CatalogEvent::Resolved(artifact));
            }
            Err(e) => {
                let _ = tx.send(CatalogEvent::Error(format!("{e:#}")));
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_render_install_idle_does_not_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = InstallState::Idle;
        let result = terminal.draw(|f| {
            render_install_screen(f, &mut state, f.area());
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_install_downloading_does_not_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = InstallState::Downloading {
            progress: 0.5,
            label: "50 MB / 100 MB".to_owned(),
        };
        let result = terminal.draw(|f| {
            render_install_screen(f, &mut state, f.area());
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_install_failed_does_not_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = InstallState::Failed {
            message: "Connection refused".to_owned(),
        };
        let result = terminal.draw(|f| {
            render_install_screen(f, &mut state, f.area());
        });
        assert!(result.is_ok());
    }
}
