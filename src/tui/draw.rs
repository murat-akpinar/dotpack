//! Rendering, and nothing else. Colors are terminal palette constants — a ricing tool
//! that overrides the user's own theme would be a joke (tui.md §6).

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap};

use super::{App, Screen};
use crate::apply::Plan;
use crate::bundle::Row;
use crate::manifest::Mode;
use crate::paths;

/// The screens are drawn at 64 columns in the design and the footers are wider than that.
const MIN: (u16, u16) = (80, 24);

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    // Phase 0: neither quit nor a compressed layout. Say what is missing and keep
    // running — a resize redraws, and nothing the user was doing is lost.
    if area.width < MIN.0 || area.height < MIN.1 {
        f.render_widget(
            Paragraph::new(format!(
                "terminal is {}x{} — dotpack needs {}x{}",
                area.width, area.height, MIN.0, MIN.1
            ))
            .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    main_screen(f, app, area);
    match &app.screen {
        Screen::Main => {}
        Screen::Plan {
            plan, scroll, hook, ..
        } => plan_screen(f, plan, *scroll, *hook, area),
        Screen::Confirm {
            question, lines, ..
        } => box_over(f, area, " confirm ", question, lines, "y apply   n cancel"),
        Screen::Prompt { title, input } => box_over(
            f,
            area,
            " add ",
            title,
            &[format!("> {input}▏")],
            "↵ fetch   esc cancel",
        ),
        Screen::Collect(wizard) => super::collect::render(f, wizard, area),
        Screen::Help => help(f, area),
    }
}

// --- main screen start ---

fn main_screen(f: &mut Frame, app: &App, area: Rect) {
    let title = match &app.status {
        Some(status) => format!(" dotpack — {status} "),
        None => " dotpack ".to_string(),
    };
    let block = Block::bordered().title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let [list, detail, footer] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(6),
        Constraint::Length(1),
    ])
    .areas(inner);

    if app.rows.is_empty() {
        f.render_widget(
            Paragraph::new("no bundles — `a` adds one, `c` collects this machine").style(dim()),
            list,
        );
    } else {
        let items: Vec<ListItem> = app
            .rows
            .iter()
            .map(|r| ListItem::new(row_line(r)))
            .collect();
        let mut state = ListState::default().with_selected(Some(app.cursor));
        f.render_stateful_widget(
            List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
            list,
            &mut state,
        );
        f.render_widget(Paragraph::new(detail_lines(app)), detail);
    }
    f.render_widget(
        Paragraph::new(Line::from(vec![
            key("↵"),
            Span::raw("switch  "),
            key("a"),
            Span::raw("add  "),
            key("c"),
            Span::raw("collect  "),
            key("s"),
            Span::raw("sync  "),
            key("d"),
            Span::raw("delete  "),
            key("-"),
            Span::raw("back  "),
            key("?"),
            Span::raw("help  "),
            key("q"),
            Span::raw("quit"),
        ])),
        footer,
    );
}

fn row_line(row: &Row) -> Line<'static> {
    let bundle = match &row.bundle {
        Ok(b) => b,
        Err(e) => {
            return Line::from(vec![
                Span::styled("✗ ", Style::default().fg(Color::Red)),
                Span::raw(format!("{:<22} {e}", row.name)),
            ]);
        }
    };
    let manifest = &bundle.manifest;
    let mut state = Vec::new();
    if row.active {
        state.push("active".to_string());
    }
    if manifest.mode == Mode::External {
        state.push("external".to_string());
    }
    if row.detached > 0 {
        state.push(format!("{} detached", row.detached));
    }

    let mut spans = vec![
        Span::styled(
            if row.active { "● " } else { "○ " },
            Style::default().fg(if row.active {
                Color::Green
            } else {
                Color::Reset
            }),
        ),
        Span::raw(format!(
            "{:<22} {:<9} {:>3} pkgs  ",
            row.name,
            format!("{:?}", manifest.wm).to_lowercase(),
            manifest.packages.count()
        )),
        Span::styled(state.join(" · "), Style::default().fg(Color::Yellow)),
    ];
    // The secret counter is the one thing watching a bundle you edit every day.
    if row.secrets > 0 {
        spans.push(Span::styled(
            format!(" ⚠{}", row.secrets),
            Style::default().fg(Color::Red),
        ));
    }
    Line::from(spans)
}

fn detail_lines(app: &App) -> Vec<Line<'static>> {
    let Some(row) = app.selected() else {
        return Vec::new();
    };
    let mut lines = vec![Line::from(Span::styled(
        "─".repeat(0).to_string() + &row.name,
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    let Ok(bundle) = &row.bundle else {
        lines.push(Line::from(Span::styled(
            row.path.display().to_string(),
            dim(),
        )));
        return lines;
    };
    let m = &bundle.manifest;
    if !m.description.is_empty() {
        lines.push(Line::from(m.description.clone()));
    }
    let mut facts = vec![format!("v{}", m.version)];
    facts.extend(m.license.clone());
    facts.extend(m.author.clone());
    facts.push(paths::contract(&row.path));
    lines.push(Line::from(Span::styled(facts.join(" · "), dim())));

    let dirs = bundle.config_dirs();
    if !dirs.is_empty() {
        let shown = dirs.len().min(5);
        let mut text = format!("config: {}", dirs[..shown].join(", "));
        if dirs.len() > shown {
            text.push_str(&format!("   +{}", dirs.len() - shown));
        }
        lines.push(Line::from(Span::styled(text, dim())));
    }
    lines
}

// --- main screen end ---

// --- plan screen start ---

fn plan_screen(f: &mut Frame, plan: &Plan, scroll: u16, hook: Option<usize>, area: Rect) {
    // Invariant 5: someone else's script, read before it is approved. It gets the whole
    // screen, because a script skimmed in four lines is a script nobody read.
    if let Some(index) = hook
        && let Some(shown) = plan.hooks.get(index)
    {
        let block = Block::bordered().title(format!(" {} · {} ", shown.path, shown.when));
        let inner = block.inner(area);
        f.render_widget(Clear, area);
        f.render_widget(block, area);
        let [body, footer] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
        f.render_widget(
            Paragraph::new(shown.script.clone()).scroll((scroll, 0)),
            body,
        );
        f.render_widget(
            footer_line(&[("↑↓", "scroll"), ("esc", "back to the plan")]),
            footer,
        );
        return;
    }

    let block = Block::bordered().title(format!(" switch → {} ", plan.name));
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    let [body, footer] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    let mut lines: Vec<Line> = Vec::new();
    let p = &plan.packages;
    if !p.is_empty() {
        lines.push(head("PACKAGES"));
        if !p.repo.is_empty() {
            lines.push(item(
                Color::Green,
                "+",
                format!("{} from repos   {}", p.repo.len(), p.repo.join(" ")),
            ));
        }
        if !p.aur.is_empty() {
            lines.push(item(
                Color::Green,
                "+",
                format!("{} from the AUR  {}", p.aur.len(), p.aur.join(" ")),
            ));
        }
        if !p.unknown.is_empty() {
            lines.push(item(
                Color::Yellow,
                "?",
                format!("no repo has {} — trying the AUR", p.unknown.join(", ")),
            ));
        }
        if p.helper.is_none() && !p.aur.is_empty() {
            lines.push(item(
                Color::Red,
                "⚠",
                "no AUR helper (paru/yay/pikaur/trizen) — those are skipped".into(),
            ));
        }
        lines.push(Line::default());
    }

    for (title, mark, color, rows) in [
        ("ROLES", "·", Color::Reset, &plan.roles),
        ("FILES", "+", Color::Green, &plan.place),
        ("", "↓", Color::Green, &plan.copy),
        ("", "−", Color::Yellow, &plan.remove),
        ("", "⚠", Color::Red, &plan.detached),
        ("SERVICES", "", Color::Reset, &plan.services),
        ("BY HAND", "→", Color::Yellow, &plan.manual),
        ("WARNINGS", "⚠", Color::Red, &plan.warnings),
    ] {
        if rows.is_empty() {
            continue;
        }
        if !title.is_empty() {
            lines.push(head(title));
        }
        lines.extend(rows.iter().map(|r| item(color, mark, r.clone())));
        if !title.is_empty() || rows.len() > 1 {
            lines.push(Line::default());
        }
    }

    for h in &plan.hooks {
        lines.push(head("HOOK"));
        lines.push(item(
            Color::Yellow,
            "!",
            format!("{} ({})   [h] show contents", h.path, h.when),
        ));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "nothing to do — it is already what is on the machine",
            dim(),
        )));
    }

    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        body,
    );
    f.render_widget(
        footer_line(&[
            ("↵", "apply"),
            ("h", "hook"),
            ("↑↓", "scroll"),
            ("esc", "cancel"),
        ]),
        footer,
    );
}

// --- plan screen end ---

fn box_over(f: &mut Frame, area: Rect, title: &str, question: &str, lines: &[String], keys: &str) {
    let height = (lines.len() as u16 + 6).min(area.height);
    let area = centered(area, 72.min(area.width), height);
    let block = Block::bordered().title(title.to_string());
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let mut body = vec![Line::from(question.to_string()), Line::default()];
    body.extend(
        lines
            .iter()
            .map(|l| Line::from(Span::styled(l.clone(), dim()))),
    );
    let [text, footer] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    f.render_widget(Paragraph::new(body).wrap(Wrap { trim: true }), text);
    f.render_widget(
        Paragraph::new(Span::styled(keys.to_string(), dim())),
        footer,
    );
}

fn help(f: &mut Frame, area: Rect) {
    // tui.md §7. The global half never changes meaning; the letters belong to a screen
    // and that screen's footer prints them, which is why they are not repeated here.
    let keys = [
        ("j k ↓ ↑", "navigate"),
        ("g G", "top / bottom"),
        ("space", "tick / untick"),
        ("enter", "confirm, advance — never destructive"),
        ("esc", "back, cancel"),
        ("tab shift-tab", "forward / back in the wizard"),
        ("/", "search"),
        ("?", "this window"),
        ("q ctrl-c", "quit"),
    ];
    let lines: Vec<String> = keys
        .iter()
        .map(|(k, what)| format!("  {k:<16}{what}"))
        .collect();
    box_over(
        f,
        area,
        " keys ",
        "the same on every screen:",
        &lines,
        "any key closes",
    );
}

// --- bits start ---

fn head(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().add_modifier(Modifier::BOLD),
    ))
}

fn item(color: Color, mark: &str, text: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {mark:<2}"), Style::default().fg(color)),
        Span::raw(text),
    ])
}

fn key(k: &str) -> Span<'static> {
    Span::styled(
        format!("{k} "),
        Style::default().add_modifier(Modifier::BOLD),
    )
}

fn footer_line(keys: &[(&str, &str)]) -> Paragraph<'static> {
    Paragraph::new(Line::from(
        keys.iter()
            .flat_map(|(k, what)| [key(k), Span::raw(format!("{what}  "))])
            .collect::<Vec<_>>(),
    ))
}

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [_, middle, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [_, middle, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width),
        Constraint::Fill(1),
    ])
    .areas(middle);
    middle
}

// --- bits end ---
