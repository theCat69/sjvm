use std::{path::PathBuf, sync::mpsc};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph},
};

use crate::core::jdk_catalog::{ArtifactInfo, Vendor};

// ---------------------------------------------------------------------------
// State types
// ---------------------------------------------------------------------------

/// Represents the current state of the install screen flow.
#[derive(Clone)]
pub(crate) enum InstallState {
    /// User is selecting a JDK vendor. `selected` is 0=OpenJDK, 1=GraalVM.
    VendorPicker { selected: usize },
    /// Fetching the list of available versions from the vendor API.
    FetchingVersions { vendor: Vendor },
    /// User is picking a version from the fetched list.
    VersionPicker {
        vendor: Vendor,
        versions: Vec<u16>,
        selected: usize,
    },
    /// Resolving artifact metadata for the chosen vendor + version.
    #[allow(dead_code)]
    FetchingArtifact { vendor: Vendor, version: u16 },
    /// Artifact metadata is ready; user confirms download.
    ArtifactReady { artifact: ArtifactInfo },
    /// Download is in progress.
    Downloading { downloaded: u64, total: Option<u64> },
    /// JDK was successfully installed at `jdk_path`.
    Installed { jdk_path: PathBuf },
    /// An error occurred.
    Failed { message: String },
}

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

/// Events emitted by the background download thread.
pub(crate) enum DownloadEvent {
    Progress { downloaded: u64, total: Option<u64> },
    Done { jdk_dir: PathBuf },
    Error { message: String },
}

/// Events emitted by the background catalog-fetch thread.
pub(crate) enum CatalogEvent {
    Resolved(ArtifactInfo),
    Error(String),
}

/// Events emitted by the background versions-fetch thread.
pub(crate) enum VersionsEvent {
    Fetched(Vec<u16>),
    Error(String),
}

// ---------------------------------------------------------------------------
// Human-readable byte formatter
// ---------------------------------------------------------------------------

/// Formats a byte count as a human-readable string (B / KB / MB / GB).
pub(crate) fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// ---------------------------------------------------------------------------
// Vendor label helper
// ---------------------------------------------------------------------------

fn vendor_label(vendor: &Vendor) -> &'static str {
    match vendor {
        Vendor::OpenJdk => "OpenJDK (Adoptium)",
        Vendor::GraalVm => "GraalVM CE",
    }
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Renders the install screen into `area`, delegating to the appropriate sub-renderer
/// based on the current `state`.
///
/// `install_vendor` is the vendor selected by the user (set after `VendorPicker`)
/// and is used to display the vendor name in the block title.
pub(crate) fn render_install_screen(
    f: &mut Frame,
    state: &mut InstallState,
    area: ratatui::layout::Rect,
    install_vendor: Option<&Vendor>,
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

    // Build the install block title: show vendor when one has been selected.
    let block_title = match install_vendor {
        Some(v) => format!(" Install JDK — {} ", vendor_label(v)),
        None => " Install JDK ".to_owned(),
    };

    match state {
        InstallState::VendorPicker { selected } => {
            let selected = *selected;
            render_header(f, chunks[0], "Select Vendor", &block_title);
            render_vendor_list(f, chunks[1], selected);
            render_empty_gauge(f, chunks[2]);
            render_help(
                f,
                chunks[3],
                "\u{2191}/\u{2193}: Select   Enter: Choose   Ctrl+C: Back",
            );
        }
        InstallState::FetchingVersions { .. } => {
            render_header(f, chunks[0], "Fetching Versions", &block_title);
            let content = Paragraph::new(Line::from("Fetching available versions..."))
                .block(Block::default().borders(Borders::ALL).title("Status"));
            f.render_widget(content, chunks[1]);
            render_empty_gauge(f, chunks[2]);
            render_help(f, chunks[3], "Please wait...   Ctrl+C: Back");
        }
        InstallState::VersionPicker {
            versions, selected, ..
        } => {
            let selected = *selected;
            render_header(f, chunks[0], "Select Version", &block_title);
            render_version_list(f, chunks[1], versions, selected);
            render_empty_gauge(f, chunks[2]);
            render_help(
                f,
                chunks[3],
                "\u{2191}/\u{2193}: Navigate   Enter: Install   Ctrl+C: Back",
            );
        }
        InstallState::FetchingArtifact { version, .. } => {
            let version = *version;
            render_header(f, chunks[0], "Resolving Artifact", &block_title);
            let content = Paragraph::new(Line::from(format!("Resolving JDK {version}...")))
                .block(Block::default().borders(Borders::ALL).title("Status"));
            f.render_widget(content, chunks[1]);
            render_empty_gauge(f, chunks[2]);
            render_help(f, chunks[3], "Please wait...   Ctrl+C: Back");
        }
        InstallState::ArtifactReady { artifact } => {
            render_header(f, chunks[0], "Version Ready", &block_title);
            let text = vec![
                Line::from(format!("File:  {}", artifact.filename)),
                Line::from(format!("URL:   {}", artifact.download_url)),
            ];
            let content = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title("Artifact"));
            f.render_widget(content, chunks[1]);
            render_empty_gauge(f, chunks[2]);
            render_help(f, chunks[3], "Enter: Download   Ctrl+C: Back");
        }
        InstallState::Downloading { downloaded, total } => {
            let downloaded = *downloaded;
            let total = *total;
            let progress = total
                .map(|t| (downloaded as f64 / t as f64).clamp(0.0, 1.0))
                .unwrap_or(0.0);
            let label = if let Some(t) = total {
                format!("{} / {}", format_bytes(downloaded), format_bytes(t))
            } else {
                format!("{} downloaded", format_bytes(downloaded))
            };
            render_header(f, chunks[0], "Downloading", &block_title);
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
            render_header(f, chunks[0], "Installed", &block_title);
            let content = Paragraph::new(Line::from(vec![
                ratatui::text::Span::styled(
                    "\u{2705} Installed: ",
                    Style::default().fg(Color::Green),
                ),
                ratatui::text::Span::raw(path_str),
            ]))
            .block(Block::default().borders(Borders::ALL).title("Done"));
            f.render_widget(content, chunks[1]);
            render_empty_gauge(f, chunks[2]);
            render_help(f, chunks[3], "Press y to switch   n/Ctrl+C to skip");
        }
        InstallState::Failed { message } => {
            let message = message.clone();
            render_header(f, chunks[0], "Error", &block_title);
            let content = Paragraph::new(Line::from(vec![
                ratatui::text::Span::styled("\u{274c} Error: ", Style::default().fg(Color::Red)),
                ratatui::text::Span::raw(message),
            ]))
            .block(Block::default().borders(Borders::ALL).title("Error"));
            f.render_widget(content, chunks[1]);
            render_empty_gauge(f, chunks[2]);
            render_help(f, chunks[3], "Ctrl+C: Back");
        }
    }
}

// ---------------------------------------------------------------------------
// Private render helpers
// ---------------------------------------------------------------------------

fn render_header(f: &mut Frame, area: ratatui::layout::Rect, title: &str, block_title: &str) {
    let header = Paragraph::new(Line::from(title.to_owned())).block(
        Block::default()
            .borders(Borders::ALL)
            .title(block_title.to_owned()),
    );
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

fn render_version_list(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    versions: &[u16],
    selected: usize,
) {
    let items: Vec<ListItem> = versions
        .iter()
        .map(|v| ListItem::new(Line::from(v.to_string())))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Version"))
        .highlight_style(Style::default().fg(Color::Cyan))
        .highlight_symbol(">> ");

    let mut list_state = ListState::default();
    list_state.select(if versions.is_empty() {
        None
    } else {
        Some(selected)
    });
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

/// Spawns a background thread to fetch available versions for a vendor.
///
/// Results are sent via `tx`.
pub(crate) fn spawn_versions_fetch(vendor: Vendor, tx: mpsc::Sender<VersionsEvent>) {
    use crate::core::jdk_catalog::fetch_available_versions;

    std::thread::spawn(move || match fetch_available_versions(&vendor) {
        Ok(versions) => {
            let _ = tx.send(VersionsEvent::Fetched(versions));
        }
        Err(e) => {
            let _ = tx.send(VersionsEvent::Error(format!("{e:#}")));
        }
    });
}

/// Spawns a background thread to download and install a JDK.
///
/// Progress events are sent via `tx` until the operation completes or fails.
pub(crate) fn spawn_download(
    artifact: ArtifactInfo,
    dest_dir: PathBuf,
    tx: mpsc::Sender<DownloadEvent>,
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
            let _ = tx_progress.send(DownloadEvent::Progress { downloaded, total });
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
/// The result is sent via `tx`.
pub(crate) fn spawn_catalog_fetch(vendor: Vendor, version: u16, tx: mpsc::Sender<CatalogEvent>) {
    use crate::core::jdk_catalog::{detect_arch, detect_os, resolve_artifact};

    std::thread::spawn(move || {
        let result = (|| -> anyhow::Result<ArtifactInfo> {
            let os = detect_os()?;
            let arch = detect_arch()?;
            resolve_artifact(&vendor, version, &os, &arch)
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
    fn test_render_install_vendor_picker_does_not_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = InstallState::VendorPicker { selected: 0 };
        let result = terminal.draw(|f| {
            render_install_screen(f, &mut state, f.area(), None);
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_install_downloading_does_not_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = InstallState::Downloading {
            downloaded: 52_428_800,
            total: Some(104_857_600),
        };
        let result = terminal.draw(|f| {
            render_install_screen(f, &mut state, f.area(), Some(&Vendor::OpenJdk));
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_install_downloading_unknown_total_does_not_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = InstallState::Downloading {
            downloaded: 1_048_576,
            total: None,
        };
        let result = terminal.draw(|f| {
            render_install_screen(f, &mut state, f.area(), Some(&Vendor::GraalVm));
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
            render_install_screen(f, &mut state, f.area(), None);
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_install_fetching_versions_does_not_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = InstallState::FetchingVersions {
            vendor: Vendor::OpenJdk,
        };
        let result = terminal.draw(|f| {
            render_install_screen(f, &mut state, f.area(), Some(&Vendor::OpenJdk));
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_install_version_picker_does_not_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = InstallState::VersionPicker {
            vendor: Vendor::OpenJdk,
            versions: vec![8, 11, 17, 21],
            selected: 2,
        };
        let result = terminal.draw(|f| {
            render_install_screen(f, &mut state, f.area(), Some(&Vendor::OpenJdk));
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_install_installed_shows_switch_prompt() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = InstallState::Installed {
            jdk_path: PathBuf::from("/usr/lib/jvm/temurin-21-jdk"),
        };
        terminal
            .draw(|f| {
                render_install_screen(f, &mut state, f.area(), Some(&Vendor::OpenJdk));
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer.content();

        // The help bar should contain the switch prompt text.
        let has_y_prompt = content
            .iter()
            .any(|cell: &ratatui::buffer::Cell| cell.symbol().contains("y"));
        assert!(has_y_prompt, "Installed state should show 'y' prompt");
    }

    #[test]
    fn test_vendor_label_in_block_title() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = InstallState::FetchingVersions {
            vendor: Vendor::OpenJdk,
        };
        terminal
            .draw(|f| {
                render_install_screen(f, &mut state, f.area(), Some(&Vendor::OpenJdk));
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer.content();

        // The block title is " Install JDK — OpenJDK (Adoptium) ".
        // Collect all rendered symbols into one string and search for the vendor substring.
        let rendered: String = content.iter().map(|cell| cell.symbol()).collect();
        assert!(
            rendered.contains("OpenJDK"),
            "Block title should contain 'OpenJDK', got rendered buffer without it"
        );
    }

    // --- format_bytes ---

    #[test]
    fn test_format_bytes_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1023), "1023 B");
    }

    #[test]
    fn test_format_bytes_kilobytes() {
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(2048), "2.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
    }

    #[test]
    fn test_format_bytes_megabytes() {
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(52 * 1024 * 1024), "52.0 MB");
    }

    #[test]
    fn test_format_bytes_gigabytes() {
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_bytes(2 * 1024 * 1024 * 1024), "2.0 GB");
    }
}
