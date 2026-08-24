//! M6 — the face. Every screen here calls the same function the CLI calls: nothing under
//! `tui/` knows how to place a link, install a package or write a bundle, and the plain
//! output a job prints is `main.rs`'s own `show` / `report`.

mod collect;
mod draw;

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::apply;
use crate::bundle::{self, Bundle, Row};

pub struct App {
    pub rows: Vec<Row>,
    pub cursor: usize,
    pub screen: Screen,
    /// What the last job did. It sits in the title until the next one replaces it.
    pub status: Option<String>,
    job: Option<Job>,
    quit: bool,
    /// One worker thread's worth of scanning, and the channel it answers on. The event
    /// loop is already spinning on `event::poll`'s timeout, so draining this is free.
    tx: Sender<collect::Msg>,
    rx: Receiver<collect::Msg>,
}

pub enum Screen {
    Main,
    Plan {
        plan: Box<apply::Plan>,
        root: PathBuf,
        scroll: u16,
        /// The hook whose source is on screen — invariant 5's window.
        hook: Option<usize>,
    },
    Confirm {
        question: String,
        lines: Vec<String>,
        job: Job,
    },
    Prompt {
        title: String,
        input: String,
    },
    Collect(Box<collect::Wizard>),
    Help,
}

/// Work that must not happen inside the alternate screen: it asks for a sudo password, or
/// it streams output already better than anything we would redraw (tui.md §5).
pub enum Job {
    Switch {
        root: PathBuf,
        plan: Box<apply::Plan>,
    },
    Deactivate,
    Sync,
    Delete(String),
    Add(String),
    Write {
        collected: Box<crate::scan::Collected>,
        out: PathBuf,
        git: bool,
    },
}

pub fn run() -> Result<()> {
    // `ratatui::init()` installs the panic hook that restores the terminal before the
    // message is printed. That hook is non-negotiable (tui.md §6) and writing our own
    // would be the same code with our name on it.
    let mut terminal = ratatui::init();
    let mut app = App::new()?;

    let outcome = loop {
        if let Err(e) = terminal.draw(|f| draw::render(f, &app)) {
            break Err(e.into());
        }
        match event::poll(Duration::from_millis(100)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => app.handle(key),
                Ok(_) => {}
                Err(e) => break Err(e.into()),
            },
            Ok(false) => {}
            Err(e) => break Err(e.into()),
        }
        app.worker();
        if let Some(job) = app.job.take() {
            ratatui::restore();
            app.status = Some(match job.run() {
                Ok(said) => said,
                Err(e) => {
                    println!("error: {e:#}");
                    "failed — see above".into()
                }
            });
            pause();
            terminal = ratatui::init();
            if let Err(e) = app.reload() {
                break Err(e);
            }
        }
        if app.quit {
            break Ok(());
        }
    };

    ratatui::restore();
    outcome
}

/// Whatever just ran printed something worth reading, and the alternate screen eats it
/// the moment we go back in.
fn pause() {
    println!("\n[press enter to continue]");
    let _ = std::io::stdin().read_line(&mut String::new());
}

impl Job {
    /// Runs with the terminal restored, and prints the way the CLI prints.
    fn run(self) -> Result<String> {
        Ok(match self {
            Job::Switch { root, plan } => {
                let bundle = Bundle::open(root)?;
                let name = plan.name.clone();
                crate::report(apply::switch(&bundle, *plan)?);
                format!("switched to {name}")
            }
            Job::Deactivate => {
                crate::report(apply::deactivate()?);
                "nothing is active now".into()
            }
            Job::Sync => {
                crate::report(apply::sync(false)?);
                "synced".into()
            }
            Job::Delete(name) => {
                apply::remove_bundle(&name)?;
                format!("{name} is out of the store")
            }
            Job::Add(source) => {
                let name = crate::into_store(&crate::source::parse(&source)?, None)?;
                format!("{name} added — ↵ switches to it")
            }
            // 5/5 is the only step that touches the disk, and this is it (tui.md §4.4).
            Job::Write {
                collected,
                out,
                git,
            } => {
                for note in apply::write::write_bundle(&collected, &out, git)? {
                    println!("  {note}");
                }
                println!("wrote {} — {} files", out.display(), collected.files.len());
                format!("wrote {}", crate::paths::contract(&out))
            }
        })
    }
}

// --- state start ---

impl App {
    fn new() -> Result<Self> {
        let (tx, rx) = channel();
        Ok(Self {
            rows: bundle::rows()?,
            cursor: 0,
            screen: Screen::Main,
            status: None,
            job: None,
            quit: false,
            tx,
            rx,
        })
    }

    /// A scan came back from the worker. Anything arriving for a wizard the user has
    /// already left is dropped — it answers a question nobody is asking any more.
    fn worker(&mut self) {
        while let Ok(message) = self.rx.try_recv() {
            if let Screen::Collect(wizard) = &mut self.screen {
                wizard.apply(message);
            }
        }
    }

    /// After a job the store and the ledger have both moved. The cursor is kept on the
    /// same *name* where there still is one — a bundle that was deleted must not leave
    /// the cursor pointing at whatever slid into its index.
    fn reload(&mut self) -> Result<()> {
        let was = self.selected().map(|r| r.name.clone());
        self.rows = bundle::rows()?;
        self.cursor = was
            .and_then(|name| self.rows.iter().position(|r| r.name == name))
            .unwrap_or(0)
            .min(self.rows.len().saturating_sub(1));
        self.screen = Screen::Main;
        Ok(())
    }

    pub fn selected(&self) -> Option<&Row> {
        self.rows.get(self.cursor)
    }

    fn handle(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        match self.screen {
            Screen::Collect(_) => self.collect_key(key),
            Screen::Help => self.screen = Screen::Main,
            Screen::Prompt { .. } => self.prompt_key(key),
            Screen::Confirm { .. } => self.confirm_key(key),
            Screen::Plan { .. } => self.plan_key(key),
            Screen::Main => self.main_key(key),
        }
    }

    fn main_key(&mut self, key: KeyEvent) {
        let last = self.rows.len().saturating_sub(1);
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.cursor = (self.cursor + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char('g') => self.cursor = 0,
            KeyCode::Char('G') => self.cursor = last,
            KeyCode::Char('?') => self.screen = Screen::Help,
            // `enter` is never destructive (invariant 7): it opens the plan, and the
            // plan's own `enter` applies it.
            KeyCode::Enter => {
                if let Some(root) = self.selected().map(|r| r.path.clone()) {
                    self.open_plan(root);
                }
            }
            KeyCode::Char('-') => self.previous(),
            KeyCode::Char('a') => {
                self.screen = Screen::Prompt {
                    title: "add — github:user/repo, a git URL, or a path".into(),
                    input: String::new(),
                }
            }
            KeyCode::Char('s') => self.sync(),
            KeyCode::Char('d') => self.delete(),
            KeyCode::Char('c') => self.start_collect(),
            _ => {}
        }
    }

    fn start_collect(&mut self) {
        match crate::scan::wm::detect() {
            Some(wm) => self.screen = Screen::Collect(Box::new(collect::Wizard::new(wm, &self.tx))),
            None => self.status = Some("could not tell which WM this is".into()),
        }
    }

    fn collect_key(&mut self, key: KeyEvent) {
        let Screen::Collect(wizard) = &mut self.screen else {
            return;
        };
        // The review step's `enter` is the only key in the wizard that leads to a write.
        if key.code == KeyCode::Enter
            && wizard.step == collect::Step::Review
            && let Some((collected, out, git)) = wizard.result()
        {
            self.screen = Screen::Main;
            self.job = Some(Job::Write {
                collected: Box::new(collected),
                out,
                git,
            });
            return;
        }
        if wizard.key(key, &self.tx) {
            // Phase 0: a half-finished wizard is not saved. `state.toml` is the only state
            // file there is, and the scan it would restore is a re-run away.
            self.screen = Screen::Main;
        }
    }

    fn plan_key(&mut self, key: KeyEvent) {
        let Screen::Plan { scroll, hook, .. } = &mut self.screen else {
            return;
        };
        match key.code {
            // Inside the hook window `esc` closes the window, not the plan.
            KeyCode::Esc | KeyCode::Char('q') if hook.is_some() => {
                *hook = None;
                *scroll = 0;
            }
            KeyCode::Esc | KeyCode::Char('q') => self.screen = Screen::Main,
            KeyCode::Char('j') | KeyCode::Down => *scroll = scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => *scroll = scroll.saturating_sub(1),
            KeyCode::Char('g') => *scroll = 0,
            KeyCode::Char('h') => {
                *hook = match hook {
                    Some(_) => None,
                    None => Some(0),
                };
                *scroll = 0;
            }
            KeyCode::Enter if hook.is_none() => {
                if let Screen::Plan { plan, root, .. } =
                    std::mem::replace(&mut self.screen, Screen::Main)
                {
                    self.job = Some(Job::Switch { root, plan });
                }
            }
            _ => {}
        }
    }

    fn confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                if let Screen::Confirm { job, .. } =
                    std::mem::replace(&mut self.screen, Screen::Main)
                {
                    self.job = Some(job);
                }
            }
            _ => self.screen = Screen::Main,
        }
    }

    fn prompt_key(&mut self, key: KeyEvent) {
        let Screen::Prompt { input, .. } = &mut self.screen else {
            return;
        };
        match key.code {
            KeyCode::Esc => self.screen = Screen::Main,
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(c) => input.push(c),
            KeyCode::Enter if input.trim().is_empty() => self.screen = Screen::Main,
            KeyCode::Enter => {
                if let Screen::Prompt { input, .. } =
                    std::mem::replace(&mut self.screen, Screen::Main)
                {
                    self.job = Some(Job::Add(input.trim().to_string()));
                }
            }
            _ => {}
        }
    }

    /// ponytail: `apply::plan` forks `pacman -T` and the UI is frozen for that one call.
    /// The worker thread the design asks for is the collect scan's, which is hundreds of
    /// forks; adding it here would be a thread to hide 200ms.
    fn open_plan(&mut self, root: PathBuf) {
        match Bundle::open(&root).and_then(|b| apply::plan(&b, apply::Options::default())) {
            Ok(plan) => {
                self.screen = Screen::Plan {
                    plan: Box::new(plan),
                    root,
                    scroll: 0,
                    hook: None,
                }
            }
            Err(e) => self.status = Some(format!("{e}")),
        }
    }

    /// `-`, and with no previous bundle it offers what `use -` does: remove every link
    /// and leave none active.
    fn previous(&mut self) {
        let ledger = match apply::ledger::Ledger::load() {
            Ok(l) => l,
            Err(e) => {
                self.status = Some(format!("{e}"));
                return;
            }
        };
        match (ledger.previous, ledger.active) {
            (Some(name), _) => self.open_plan(crate::paths::store().join(name)),
            (None, Some(_)) => {
                self.screen = Screen::Confirm {
                    question: "no previous bundle — remove every link and leave none active?"
                        .into(),
                    lines: ledger.links.iter().map(|l| l.target.clone()).collect(),
                    job: Job::Deactivate,
                }
            }
            (None, None) => self.status = Some("nothing is active".into()),
        }
    }

    fn sync(&mut self) {
        let Some(row) = self.rows.iter().find(|r| r.active) else {
            self.status = Some("nothing is active".into());
            return;
        };
        if row.detached == 0 {
            self.status = Some("every link is where it should be".into());
            return;
        }
        let lines = match (apply::ledger::Ledger::load(), &row.bundle) {
            (Ok(ledger), Ok(bundle)) => ledger
                .links
                .iter()
                .filter(|e| apply::links::state(e, &bundle.root) != apply::links::State::Linked)
                .map(|e| e.target.clone())
                .collect(),
            _ => Vec::new(),
        };
        self.screen = Screen::Confirm {
            question: "write these back into the bundle and re-link?".into(),
            lines,
            job: Job::Sync,
        };
    }

    /// Phase 0: `y/n`, not typing the name. `rm` refuses while the bundle is active, a
    /// local bundle is only a link in the store and a cloned one clones again — nothing
    /// here is the kind of loss that earns a typed confirmation.
    fn delete(&mut self) {
        let Some(row) = self.selected() else { return };
        if row.active {
            self.status = Some("it is active — `-` goes back first".into());
            return;
        }
        self.screen = Screen::Confirm {
            question: format!("delete {} from the store?", row.name),
            lines: vec![row.path.display().to_string()],
            job: Job::Delete(row.name.clone()),
        };
    }
}

// --- state end ---

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn screen(width: u16, height: u16, app: &App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| draw::render(f, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    fn app() -> App {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example");
        let (tx, rx) = channel();
        App {
            tx,
            rx,
            rows: vec![Row {
                name: "imperative-hyprland".into(),
                bundle: Bundle::open(&root).map_err(|e| e.to_string()),
                path: root,
                active: true,
                detached: 2,
                secrets: 1,
            }],
            cursor: 0,
            screen: Screen::Main,
            status: None,
            job: None,
            quit: false,
        }
    }

    /// The main screen says the four things tui.md §2 asks it to, and the size floor is a
    /// message rather than an exit — a resize brings the screen back with nothing lost.
    #[test]
    fn main_screen_and_the_size_floor() {
        let drawn = screen(80, 24, &app());
        for wanted in [
            "imperative-hyprland",
            "hyprland",
            "active",
            "2 detached",
            "⚠1",
            "q quit",
        ] {
            assert!(drawn.contains(wanted), "{wanted} is missing from:\n{drawn}");
        }
        assert!(screen(60, 20, &app()).contains("needs 80x24"));
    }

    /// `enter` opens the plan and never applies it (invariant 7); the job only exists once
    /// the plan screen's own `enter` has been pressed.
    #[test]
    fn enter_is_never_destructive() {
        let mut app = app();
        app.screen = Screen::Plan {
            plan: Box::new(crate::apply::Plan {
                name: "b".into(),
                packages: Default::default(),
                place: vec!["~/.config/hypr".into()],
                copy: Vec::new(),
                remove: Vec::new(),
                detached: Vec::new(),
                services: Vec::new(),
                warnings: Vec::new(),
                roles: Vec::new(),
                manual: Vec::new(),
                hooks: Vec::new(),
            }),
            root: std::path::PathBuf::from("/nowhere"),
            scroll: 0,
            hook: None,
        };
        assert!(screen(80, 24, &app).contains("~/.config/hypr"));
        assert!(app.job.is_none());

        app.handle(KeyEvent::from(KeyCode::Esc));
        assert!(matches!(app.screen, Screen::Main), "esc goes back");
        assert!(app.job.is_none(), "and cancels without applying");
    }
}
