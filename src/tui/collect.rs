//! The collect wizard — tui.md §4. Five steps over one scan: everything the screens show
//! comes from `scan::collect`, which is the same call `dotpack collect` makes.
//!
//! Nothing here writes. Step 5/5 hands `apply::write_bundle()` a plan and the TUI closes
//! while it runs, exactly like a switch does.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap};

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::manifest::Wm;
use crate::paths;
use crate::scan::{self, Collected};

/// One top-level directory under `~/.config`, as step 2/5 lists it.
pub struct Dir {
    pub name: String,
    pub files: usize,
    pub bytes: u64,
    /// Deny-list hits (`gh/hosts.yml`, shell history…). The content scan is the collect
    /// scan's job; this is the cheap half, and it is what turns the row red.
    pub risky: usize,
    pub ticked: bool,
}

/// One row of step 3/5. Editing these edits the manifest that is about to be written.
pub struct Pick {
    pub name: String,
    /// Which list it goes back into: `pacman`, `yay` or `paru`.
    pub field: &'static str,
    pub reason: String,
    pub ticked: bool,
}

pub enum Msg {
    Dirs(Vec<Dir>),
    Scanned(Box<Collected>),
    Failed(String),
}

#[derive(PartialEq, Clone, Copy)]
pub enum Step {
    Identity,
    Files,
    Packages,
    Warnings,
    Review,
}

pub struct Wizard {
    pub step: Step,
    pub name: String,
    pub wm: Wm,
    pub wm_text: String,
    pub description: String,
    pub out: String,
    pub git: bool,
    /// The focused identity field, 0..4.
    pub field: usize,

    pub dirs: Vec<Dir>,
    pub picks: Vec<Pick>,
    pub scan: Option<Collected>,
    pub working: bool,
    pub error: Option<String>,

    pub cursor: usize,
    pub filter: String,
    /// What the keyboard is currently feeding. `None` is the normal state where `j` is a
    /// movement rather than a letter.
    pub input: Option<Input>,
    pub pending: String,
}

#[derive(PartialEq, Clone, Copy)]
pub enum Input {
    Filter,
    /// The scan misses what no config launches — `starship` lives in fish's prompt and
    /// nothing execs it. This is the row the user adds themselves.
    Package,
}

impl Wizard {
    pub fn new(wm: Wm, tx: &Sender<Msg>) -> Self {
        let user = std::env::var("USER").unwrap_or_else(|_| "my".into());
        let wizard = Self {
            step: Step::Identity,
            name: format!("{}-{}", user.to_ascii_lowercase(), wm_name(wm)),
            wm,
            wm_text: wm_name(wm).to_string(),
            description: String::new(),
            out: paths::contract(&paths::home().join("dotfiles")),
            git: true,
            field: 0,
            dirs: Vec::new(),
            picks: Vec::new(),
            scan: None,
            working: true,
            error: None,
            cursor: 0,
            filter: String::new(),
            input: None,
            pending: String::new(),
        };
        // "Nothing here is discovered, so there is nothing to wait for" — the walk starts
        // while the user is still filling in the name (tui.md §4.0).
        let tx = tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Dirs(walk_config(wm)));
        });
        wizard
    }

    pub fn apply(&mut self, message: Msg) {
        self.working = false;
        match message {
            Msg::Dirs(dirs) => self.dirs = dirs,
            Msg::Scanned(collected) => {
                self.picks = picks(&collected);
                self.scan = Some(*collected);
            }
            Msg::Failed(e) => self.error = Some(e),
        }
    }

    fn selection(&self) -> Vec<String> {
        self.dirs
            .iter()
            .filter(|d| d.ticked)
            .map(|d| d.name.clone())
            .collect()
    }

    /// Re-runs the whole scan on the worker thread. Everything downstream of the file
    /// selection is derived from it, so there is one call and not four (design.md §4.1).
    fn rescan(&mut self, tx: &Sender<Msg>) {
        let (selection, wm, name) = (self.selection(), self.wm, self.name.clone());
        if selection.is_empty() {
            self.error = Some("nothing ticked".into());
            return;
        }
        self.working = true;
        self.error = None;
        let tx = tx.clone();
        std::thread::spawn(move || {
            let message = match scan::collect(
                &selection,
                &[],
                Some(name),
                wm,
                crate::manifest::Mode::Symlink,
            ) {
                Ok(collected) => Msg::Scanned(Box::new(collected)),
                Err(e) => Msg::Failed(format!("{e}")),
            };
            let _ = tx.send(message);
        });
    }

    /// What the review step writes: the scanned manifest with the user's edits on top.
    pub fn result(&mut self) -> Option<(Collected, PathBuf, bool)> {
        let mut collected = self.scan.take()?;
        collected.manifest.name = self.name.clone();
        collected.manifest.description = self.description.clone();
        collected.manifest.wm = self.wm;
        let p = &mut collected.manifest.packages;
        for (field, list) in [
            ("pacman", &mut p.pacman),
            ("yay", &mut p.yay),
            ("paru", &mut p.paru),
        ] {
            list.retain(|name| {
                self.picks
                    .iter()
                    .any(|pick| pick.ticked && pick.field == field && &pick.name == name)
            });
        }
        Some((collected, paths::expand(&self.out), self.git))
    }

    // --- keys ---

    /// `true` when the wizard is done with itself and the caller should go back.
    pub fn key(&mut self, key: KeyEvent, tx: &Sender<Msg>) -> bool {
        if let Some(input) = self.input {
            return self.input_key(key, input);
        }
        match key.code {
            KeyCode::Esc => return true,
            // Phase 0: a half-finished wizard starts over. Saving it needs a second state
            // file, and `state.toml` is the only one there is.
            KeyCode::Tab => self.forward(tx),
            KeyCode::BackTab => self.back(),
            KeyCode::Char('/') if self.listing() => {
                self.input = Some(Input::Filter);
                self.filter.clear();
            }
            KeyCode::Char('+') if self.step == Step::Packages => {
                self.input = Some(Input::Package);
                self.pending.clear();
            }
            _ => match self.step {
                Step::Identity => self.identity_key(key),
                Step::Files | Step::Packages => self.list_key(key),
                Step::Warnings => self.warning_key(key, tx),
                Step::Review => self.review_key(key),
            },
        }
        false
    }

    fn listing(&self) -> bool {
        matches!(self.step, Step::Files | Step::Packages)
    }

    fn forward(&mut self, tx: &Sender<Msg>) {
        self.cursor = 0;
        self.filter.clear();
        self.step = match self.step {
            Step::Identity => {
                self.rescan(tx);
                Step::Files
            }
            // The selection is what the scan is a function of, so leaving 2/5 is the
            // moment it has to run again.
            Step::Files => {
                self.rescan(tx);
                Step::Packages
            }
            Step::Packages if self.warnings().0 + self.warnings().1 == 0 => Step::Review,
            Step::Packages => Step::Warnings,
            Step::Warnings | Step::Review => Step::Review,
        };
    }

    fn back(&mut self) {
        self.cursor = 0;
        self.filter.clear();
        self.step = match self.step {
            Step::Identity | Step::Files => Step::Identity,
            Step::Packages => Step::Files,
            Step::Warnings => Step::Packages,
            Step::Review if self.warnings().0 + self.warnings().1 == 0 => Step::Packages,
            Step::Review => Step::Warnings,
        };
    }

    fn warnings(&self) -> (usize, usize) {
        match &self.scan {
            Some(s) => (s.secrets.len(), s.dangling.len()),
            None => (0, 0),
        }
    }

    fn identity_key(&mut self, key: KeyEvent) {
        let field = self.field;
        match key.code {
            KeyCode::Down => self.field = (self.field + 1).min(3),
            KeyCode::Up => self.field = self.field.saturating_sub(1),
            KeyCode::Backspace => {
                self.text(field).pop();
            }
            // Every key is text on a form screen — a `j` in a bundle name is a `j`.
            KeyCode::Char(c) => self.text(field).push(c),
            _ => {}
        }
        // The wm field is the one with a vocabulary. An unknown value keeps the last good
        // one, so the manifest can never carry nonsense.
        if let Some(wm) = scan::wm::parse(&self.wm_text) {
            self.wm = wm;
        }
    }

    fn text(&mut self, field: usize) -> &mut String {
        match field {
            0 => &mut self.name,
            1 => &mut self.wm_text,
            2 => &mut self.description,
            _ => &mut self.out,
        }
    }

    fn list_key(&mut self, key: KeyEvent) {
        let visible = self.visible();
        let last = visible.len().saturating_sub(1);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.cursor = (self.cursor + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char('g') => self.cursor = 0,
            KeyCode::Char('G') => self.cursor = last,
            KeyCode::Char(' ') => {
                if let Some(&index) = visible.get(self.cursor) {
                    self.tick(index);
                }
            }
            KeyCode::Char('a') => self.tick_all(&visible, true),
            KeyCode::Char('n') => self.tick_all(&visible, false),
            _ => {}
        }
    }

    fn tick(&mut self, index: usize) {
        match self.step {
            Step::Files => self.dirs[index].ticked = !self.dirs[index].ticked,
            _ => self.picks[index].ticked = !self.picks[index].ticked,
        }
    }

    fn tick_all(&mut self, visible: &[usize], to: bool) {
        for &index in visible {
            match self.step {
                Step::Files => self.dirs[index].ticked = to,
                _ => self.picks[index].ticked = to,
            }
        }
    }

    /// Indices into `dirs` / `picks` that the filter lets through. `/` filters rather than
    /// jumping (Phase 0): on a list where every row is a decision, hiding the rest is the
    /// point.
    pub fn visible(&self) -> Vec<usize> {
        let matches = |name: &str| self.filter.is_empty() || name.contains(&self.filter);
        match self.step {
            Step::Files => (0..self.dirs.len())
                .filter(|&i| matches(&self.dirs[i].name))
                .collect(),
            _ => (0..self.picks.len())
                .filter(|&i| matches(&self.picks[i].name))
                .collect(),
        }
    }

    fn input_key(&mut self, key: KeyEvent, input: Input) -> bool {
        let buffer = match input {
            Input::Filter => &mut self.filter,
            Input::Package => &mut self.pending,
        };
        match key.code {
            KeyCode::Esc => {
                buffer.clear();
                self.input = None;
            }
            KeyCode::Backspace => {
                buffer.pop();
            }
            KeyCode::Char(c) => buffer.push(c),
            KeyCode::Enter => {
                if input == Input::Package && !self.pending.trim().is_empty() {
                    self.picks.push(Pick {
                        name: self.pending.trim().to_string(),
                        // By hand means from the repos; an AUR name the user knows they
                        // want is a line in the manifest they can move afterwards.
                        field: "pacman",
                        reason: "added by hand".into(),
                        ticked: true,
                    });
                    self.pending.clear();
                }
                self.input = None;
            }
            _ => {}
        }
        self.cursor = 0;
        false
    }

    /// `a` adds the directory a dangling reference points into, and re-runs the scan —
    /// which is the only way the warning can go away.
    fn warning_key(&mut self, key: KeyEvent, tx: &Sender<Msg>) {
        let Some(scan) = &self.scan else { return };
        let last = scan.dangling.len().saturating_sub(1);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.cursor = (self.cursor + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char('a') => {
                let directory = scan
                    .dangling
                    .get(self.cursor)
                    .and_then(|r| r.path.as_ref())
                    .and_then(|p| p.strip_prefix(paths::config()).ok())
                    .and_then(|p| p.components().next())
                    .map(|c| c.as_os_str().to_string_lossy().to_string());
                if let Some(name) = directory
                    && let Some(dir) = self.dirs.iter_mut().find(|d| d.name == name)
                {
                    dir.ticked = true;
                    self.rescan(tx);
                }
            }
            _ => {}
        }
    }

    fn review_key(&mut self, key: KeyEvent) {
        if let KeyCode::Char(' ') = key.code {
            self.git = !self.git;
        }
    }
}

// --- the worker's half ---

/// Every top-level directory under `~/.config`, with what it costs to ship. This is the
/// walk that has to be off the UI thread: a browser profile in there is 2252 files.
fn walk_config(wm: Wm) -> Vec<Dir> {
    let mut warnings = Vec::new();
    let pre_ticked = scan::default_selection(wm, &mut warnings);
    let mut dirs = Vec::new();
    let Ok(entries) = std::fs::read_dir(paths::config()) else {
        return dirs;
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let (mut files, mut bytes, mut risky) = (0, 0, 0);
        for file in walkdir::WalkDir::new(entry.path())
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir())
        {
            files += 1;
            bytes += file.metadata().map(|m| m.len()).unwrap_or(0);
            if let Ok(relative) = file.path().strip_prefix(paths::home())
                && scan::secrets::denied(relative).is_some()
            {
                risky += 1;
            }
        }
        let ticked = pre_ticked.contains(&name) && risky == 0;
        dirs.push(Dir {
            name,
            files,
            bytes,
            risky,
            ticked,
        });
    }
    dirs.sort_by(|a, b| a.name.cmp(&b.name));
    dirs
}

fn picks(collected: &Collected) -> Vec<Pick> {
    let reason = |name: &String| {
        collected
            .suggestions
            .iter()
            .find(|s| &s.package == name)
            .map(|s| paths::contract(std::path::Path::new(&s.reason)))
            .unwrap_or_else(|| "font / theme".into())
    };
    let p = &collected.manifest.packages;
    [("pacman", &p.pacman), ("yay", &p.yay), ("paru", &p.paru)]
        .into_iter()
        .flat_map(|(field, list)| {
            list.iter().map(move |name| Pick {
                name: name.clone(),
                field,
                reason: reason(name),
                ticked: true,
            })
        })
        .collect()
}

fn wm_name(wm: Wm) -> &'static str {
    match wm {
        Wm::Hyprland => "hyprland",
        Wm::Sway => "sway",
        Wm::I3 => "i3",
    }
}

fn size(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1 << 20) as f64),
        b if b >= 1 << 10 => format!("{} KB", b / (1 << 10)),
        b => format!("{b} B"),
    }
}

// --- drawing start ---

pub fn render(f: &mut Frame, w: &Wizard, area: Rect) {
    let (number, label) = match w.step {
        Step::Identity => (1, "Identity"),
        Step::Files => (2, "Files"),
        Step::Packages => (3, "Packages"),
        Step::Warnings => (4, "Warnings"),
        Step::Review => (5, "Review"),
    };
    let block = Block::bordered().title(format!(" collect · {number}/5 · {label} "));
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    f.render_widget(Paragraph::new(head_line(w)), header);
    match w.step {
        Step::Identity => identity(f, w, body),
        Step::Files => checklist(f, w, body, file_rows(w)),
        Step::Packages => checklist(f, w, body, package_rows(w)),
        Step::Warnings => f.render_widget(
            Paragraph::new(warning_lines(w)).wrap(Wrap { trim: false }),
            body,
        ),
        Step::Review => f.render_widget(Paragraph::new(review_lines(w)), body),
    }
    f.render_widget(Paragraph::new(Span::styled(keys(w), dim())), footer);
}

fn head_line(w: &Wizard) -> Line<'static> {
    match w.input {
        Some(Input::Filter) => return Line::from(format!("/ {}▏", w.filter)),
        Some(Input::Package) => return Line::from(format!("+ package: {}▏", w.pending)),
        None => {}
    }
    if let Some(error) = &w.error {
        return Line::from(Span::styled(error.clone(), Style::default().fg(Color::Red)));
    }
    if w.working {
        return Line::from(Span::styled("scanning…", dim()));
    }
    let text = match w.step {
        Step::Files => {
            let (dirs, files, bytes) = w
                .dirs
                .iter()
                .filter(|d| d.ticked)
                .fold((0, 0, 0), |(dirs, files, bytes), d| {
                    (dirs + 1, files + d.files, bytes + d.bytes)
                });
            format!(
                "wm: {} · selected: {dirs} folders · {files} files · {}",
                wm_name(w.wm),
                size(bytes)
            )
        }
        Step::Packages => {
            let ticked = w.picks.iter().filter(|p| p.ticked).count();
            let aur = w
                .picks
                .iter()
                .filter(|p| p.ticked && p.field != "pacman")
                .count();
            format!(
                "{} packages found · {ticked} ticked   [pacman {} · AUR {aur}]",
                w.picks.len(),
                ticked - aur
            )
        }
        _ => String::new(),
    };
    Line::from(Span::styled(text, dim()))
}

fn identity(f: &mut Frame, w: &Wizard, area: Rect) {
    let fields = [
        ("name", &w.name),
        ("wm", &w.wm_text),
        ("description", &w.description),
        ("output", &w.out),
    ];
    let lines: Vec<Line> = fields
        .iter()
        .enumerate()
        .map(|(index, (label, value))| {
            let focused = index == w.field;
            Line::from(vec![
                Span::styled(
                    format!("{} {label:<14}", if focused { "▸" } else { " " }),
                    if focused {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        dim()
                    },
                ),
                Span::raw(format!("{value}{}", if focused { "▏" } else { "" })),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), area);
}

fn checklist(f: &mut Frame, w: &Wizard, area: Rect, rows: Vec<Line<'static>>) {
    let items: Vec<ListItem> = rows.into_iter().map(ListItem::new).collect();
    let mut state = ListState::default().with_selected(Some(w.cursor));
    f.render_stateful_widget(
        List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        area,
        &mut state,
    );
}

fn file_rows(w: &Wizard) -> Vec<Line<'static>> {
    w.visible()
        .into_iter()
        .map(|index| {
            let d = &w.dirs[index];
            let mut spans = vec![Span::raw(format!(
                "[{}] {:<18} {:>5} files  {:>9}   ",
                if d.ticked { "x" } else { " " },
                d.name,
                d.files,
                size(d.bytes)
            ))];
            // ⚠ rows are red and start unticked — they have to be ticked deliberately.
            if d.risky > 0 {
                spans.push(Span::styled(
                    format!("⚠ {} on the deny-list", d.risky),
                    Style::default().fg(Color::Red),
                ));
            }
            Line::from(spans)
        })
        .collect()
}

fn package_rows(w: &Wizard) -> Vec<Line<'static>> {
    w.visible()
        .into_iter()
        .map(|index| {
            let p = &w.picks[index];
            Line::from(vec![
                Span::raw(format!(
                    "[{}] {:<26} {:<7} ",
                    if p.ticked { "x" } else { " " },
                    p.name,
                    if p.field == "pacman" { "repo" } else { "AUR" }
                )),
                // The third column is why the package is in the list, and it is the only
                // way to weed out a false positive.
                Span::styled(p.reason.clone(), dim()),
            ])
        })
        .collect()
}

fn warning_lines(w: &Wizard) -> Vec<Line<'static>> {
    let Some(scan) = &w.scan else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    if !scan.secrets.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(
                "⚠ possible secrets in {} files — none added to the bundle",
                scan.secrets.len()
            ),
            Style::default().fg(Color::Red),
        )));
        for finding in &scan.secrets {
            lines.push(Line::from(Span::styled(
                format!(
                    "   {}:{}  {}",
                    paths::contract(&finding.file),
                    finding.line,
                    finding.what
                ),
                dim(),
            )));
        }
        lines.push(Line::from(Span::styled(
            "  To include them anyway, go back to step 2 and tick them.",
            dim(),
        )));
        lines.push(Line::default());
    }
    if !scan.dangling.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(
                "⚠ {} references point outside the bundle",
                scan.dangling.len()
            ),
            Style::default().fg(Color::Yellow),
        )));
        for (index, reference) in scan.dangling.iter().enumerate() {
            let here = index == w.cursor;
            lines.push(Line::from(vec![
                Span::raw(format!(
                    "{} {}:{}  {}",
                    if here { "▸" } else { " " },
                    paths::contract(&reference.from),
                    reference.line,
                    reference.raw
                )),
                Span::styled(
                    format!(
                        "  → {:?}{}",
                        reference.verdict,
                        // Nothing to add when the file is missing here too: the reference
                        // is already dead upstream (tui.md §4.3).
                        match reference.verdict {
                            scan::refs::Verdict::Addable => "   [a] add it",
                            _ => "",
                        }
                    ),
                    dim(),
                ),
            ]));
        }
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            "Shipping kitty.conf without catppuccin.conf ships a kitty that errors on every start.",
            dim(),
        )));
    }
    lines
}

fn review_lines(w: &Wizard) -> Vec<Line<'static>> {
    let Some(scan) = &w.scan else {
        return vec![Line::from("nothing scanned yet")];
    };
    let (dirs, files, bytes) = w
        .dirs
        .iter()
        .filter(|d| d.ticked)
        .fold((0, 0, 0), |(dirs, files, bytes), d| {
            (dirs + 1, files + d.files, bytes + d.bytes)
        });
    let ticked = w.picks.iter().filter(|p| p.ticked).count();
    vec![
        Line::from(format!(
            "  {dirs} directories · {files} files · {}",
            size(bytes)
        )),
        Line::from(format!("  {ticked} packages")),
        Line::from(format!(
            "  {} components · {} warnings accepted",
            scan.manifest.components.len(),
            scan.secrets.len() + scan.dangling.len()
        )),
        Line::default(),
        Line::from(format!("  → {}", w.out)),
        Line::from(format!(
            "  [{}] git init and commit",
            if w.git { "x" } else { " " }
        )),
        Line::default(),
        Line::from(Span::styled(
            "  ↵ writes. Nothing before this step touched the disk.",
            dim(),
        )),
    ]
}

fn keys(w: &Wizard) -> &'static str {
    match (w.input, w.step) {
        (Some(Input::Filter), _) => "type to filter   ↵ keep it   esc clear",
        (Some(Input::Package), _) => "type a package name   ↵ add it   esc cancel",
        (_, Step::Identity) => "↑↓ field   tab next   esc cancel",
        (_, Step::Files) => "space select   a all   n none   / search   tab next   esc cancel",
        (_, Step::Packages) => {
            "space select   + add   a all   n none   / search   tab next   shift-tab back"
        }
        (_, Step::Warnings) => "a add the file   ↑↓ move   tab next   shift-tab back",
        (_, Step::Review) => "space git init   ↵ write   shift-tab back   esc cancel",
    }
}

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

// --- drawing end ---

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyEvent;

    fn wizard() -> Wizard {
        Wizard {
            step: Step::Files,
            name: "me-hyprland".into(),
            wm: Wm::Hyprland,
            wm_text: "hyprland".into(),
            description: String::new(),
            out: "~/dotfiles".into(),
            git: true,
            field: 0,
            dirs: vec![
                Dir {
                    name: "hypr".into(),
                    files: 12,
                    bytes: 1 << 20,
                    risky: 0,
                    ticked: true,
                },
                Dir {
                    name: "gh".into(),
                    files: 1,
                    bytes: 900,
                    risky: 1,
                    ticked: false,
                },
            ],
            picks: vec![Pick {
                name: "kitty".into(),
                field: "pacman",
                reason: "~/.config/hypr/hyprland.conf:1".into(),
                ticked: true,
            }],
            scan: None,
            working: false,
            error: None,
            cursor: 0,
            filter: String::new(),
            input: None,
            pending: String::new(),
        }
    }

    fn drawn(w: &Wizard) -> String {
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal.draw(|f| render(f, w, f.area())).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    /// The row that carries a deny-list hit is drawn as one and starts unticked: it has
    /// to be ticked deliberately (tui.md §4.1).
    #[test]
    fn a_risky_directory_is_visible_and_untouched() {
        let text = drawn(&wizard());
        assert!(text.contains("[x] hypr"), "{text}");
        assert!(text.contains("[ ] gh"), "{text}");
        assert!(text.contains("deny-list"), "{text}");
        assert!(text.contains("selected: 1 folders"), "{text}");
    }

    /// `/` filters the list rather than jumping to a match (Phase 0), and a tick still
    /// lands on the right row when the list is short.
    #[test]
    fn search_filters_and_the_tick_follows_the_filter() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut w = wizard();
        w.key(KeyEvent::from(KeyCode::Char('/')), &tx);
        w.key(KeyEvent::from(KeyCode::Char('g')), &tx);
        assert_eq!(w.visible(), vec![1], "only gh survives the filter");
        w.key(KeyEvent::from(KeyCode::Enter), &tx); // keep the filter, leave typing
        w.key(KeyEvent::from(KeyCode::Char(' ')), &tx);
        assert!(w.dirs[1].ticked, "space ticked gh, not the row above it");
        assert!(w.dirs[0].ticked);
    }

    /// The row the scan cannot produce: nothing in a config launches `starship`.
    #[test]
    fn a_package_can_be_added_by_hand() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut w = wizard();
        w.step = Step::Packages;
        for code in [
            KeyCode::Char('+'),
            KeyCode::Char('s'),
            KeyCode::Char('h'),
            KeyCode::Enter,
        ] {
            w.key(KeyEvent::from(code), &tx);
        }
        assert!(
            w.picks.iter().any(|p| p.name == "sh" && p.ticked),
            "the typed name is a ticked row, not a filter"
        );
        assert!(w.input.is_none());
    }

    /// 4/5 is skipped when it has nothing to say, forwards *and* back — a step that
    /// appears in one direction only is how a wizard traps someone.
    #[test]
    fn the_warnings_step_is_skipped_when_it_is_empty() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut w = wizard();
        w.step = Step::Packages;
        w.key(KeyEvent::from(KeyCode::Tab), &tx);
        assert!(w.step == Step::Review);
        w.key(KeyEvent::from(KeyCode::BackTab), &tx);
        assert!(w.step == Step::Packages);
    }
}
