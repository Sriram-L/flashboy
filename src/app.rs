use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use tui_textarea::{Input, Key, TextArea};

use crate::runner::{self, TLE_SECS};
use crate::store::{self, Bank, Case, RunRecord, Verdict};

pub enum Screen {
    Picker,
    Main,
    Edit,
    ConfirmDelete,
    Help,
    Message,
}

pub enum EditFocus {
    Name,
    Input,
    Expected,
}

pub struct EditState {
    pub is_new: bool,
    pub id: Option<u32>,
    pub name: String,
    pub input: TextArea<'static>,
    pub expected: TextArea<'static>,
    pub focus: EditFocus,
}

pub enum WorkerEvent {
    CompileOk { duration_ns: u64, total: usize },
    CompileFail(String),
    CaseDone { id: u32, record: RunRecord, done: usize, total: usize },
    BatchDone,
}

pub struct App {
    pub screen: Screen,
    pub should_quit: bool,
    pub cpp_files: Vec<PathBuf>,
    pub picker_state: ListState,
    pub cpp: Option<PathBuf>,
    pub bank_path: Option<PathBuf>,
    pub bank: Bank,
    pub list_state: ListState,
    pub detail_scroll: u16,
    pub status: String,
    pub message: String,
    pub busy: bool,
    pub compiler: String,
    pub tx: Sender<WorkerEvent>,
    pub rx: Receiver<WorkerEvent>,
    pub edit: Option<EditState>,
    pub dirty: bool,
}

impl App {
    pub fn new(arg: Option<PathBuf>) -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let compiler = runner::discover_compiler().unwrap_or_else(|e| e.to_string());
        let mut app = Self {
            screen: Screen::Picker,
            should_quit: false,
            cpp_files: scan_cpp(),
            picker_state: ListState::default(),
            cpp: None,
            bank_path: None,
            bank: Bank::new("program.cpp"),
            list_state: ListState::default(),
            detail_scroll: 0,
            status: format!("TLE {TLE_SECS}s · g++-12 · postcard+zstd .fbk"),
            message: String::new(),
            busy: false,
            compiler,
            tx,
            rx,
            edit: None,
            dirty: false,
        };
        if !app.cpp_files.is_empty() {
            app.picker_state.select(Some(0));
        }
        if let Some(p) = arg {
            if p.is_file() {
                app.open_cpp(p)?;
            }
        }
        Ok(app)
    }

    pub fn open_cpp(&mut self, path: PathBuf) -> Result<()> {
        let path = fs::canonicalize(&path).unwrap_or(path);
        let (bank_path, bank) = store::load_or_new(&path)?;
        self.cpp = Some(path);
        self.bank_path = Some(bank_path);
        self.bank = bank;
        self.list_state = ListState::default();
        if !self.bank.cases.is_empty() {
            self.list_state.select(Some(0));
        }
        self.detail_scroll = 0;
        self.screen = Screen::Main;
        self.status = format!(
            "{}  ·  {} cases  ·  {}",
            self.cpp.as_ref().unwrap().display(),
            self.bank.cases.len(),
            self.compiler
        );
        Ok(())
    }

    pub fn persist(&mut self) {
        if let Some(path) = &self.bank_path {
            match store::save(path, &self.bank) {
                Ok(()) => self.dirty = false,
                Err(e) => self.toast(format!("save failed: {e}")),
            }
        }
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.screen = Screen::Message;
    }

    pub fn selected_id(&self) -> Option<u32> {
        self.list_state
            .selected()
            .and_then(|i| self.bank.cases.get(i).map(|c| c.id))
    }

    pub fn selected_case(&self) -> Option<&Case> {
        self.list_state
            .selected()
            .and_then(|i| self.bank.cases.get(i))
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if self.busy && !matches!(self.screen, Screen::Edit) {
            if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
                self.should_quit = true;
            }
            return;
        }
        match self.screen {
            Screen::Picker => self.on_picker(key),
            Screen::Main => self.on_main(key),
            Screen::Edit => self.on_edit(key),
            Screen::ConfirmDelete => self.on_confirm(key),
            Screen::Help | Screen::Message => {
                self.screen = if self.cpp.is_some() {
                    Screen::Main
                } else {
                    Screen::Picker
                };
            }
        }
    }

    fn on_picker(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('?') => self.screen = Screen::Help,
            KeyCode::Down | KeyCode::Char('j') => {
                let n = self.cpp_files.len();
                if n == 0 {
                    return;
                }
                let i = self.picker_state.selected().unwrap_or(0);
                self.picker_state.select(Some((i + 1) % n));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let n = self.cpp_files.len();
                if n == 0 {
                    return;
                }
                let i = self.picker_state.selected().unwrap_or(0);
                self.picker_state.select(Some((i + n - 1) % n));
            }
            KeyCode::Enter => {
                if let Some(i) = self.picker_state.selected() {
                    if let Some(p) = self.cpp_files.get(i).cloned() {
                        if let Err(e) = self.open_cpp(p) {
                            self.toast(format!("{e:#}"));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn on_main(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => {
                if self.dirty {
                    self.persist();
                }
                self.should_quit = true;
            }
            KeyCode::Esc => {
                if self.dirty {
                    self.persist();
                }
                self.cpp = None;
                self.screen = Screen::Picker;
                self.cpp_files = scan_cpp();
            }
            KeyCode::Char('?') => self.screen = Screen::Help,
            KeyCode::Char('n') => self.begin_edit(true),
            KeyCode::Char('e') | KeyCode::Char('i') => {
                if self.selected_case().is_some() {
                    self.begin_edit(false);
                }
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                if self.selected_id().is_some() {
                    self.screen = Screen::ConfirmDelete;
                }
            }
            KeyCode::Char('r') => self.run_selected(),
            KeyCode::Char('R') | KeyCode::Char('a') => self.run_all(),
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => self.persist(),
            KeyCode::Down | KeyCode::Char('j') => self.move_sel(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_sel(-1),
            KeyCode::Char('J') => self.detail_scroll = self.detail_scroll.saturating_add(1),
            KeyCode::Char('K') => self.detail_scroll = self.detail_scroll.saturating_sub(1),
            _ => {}
        }
    }

    fn on_confirm(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(id) = self.selected_id() {
                    let idx = self.list_state.selected().unwrap_or(0);
                    self.bank.remove(id);
                    if self.bank.cases.is_empty() {
                        self.list_state.select(None);
                    } else {
                        self.list_state
                            .select(Some(idx.min(self.bank.cases.len() - 1)));
                    }
                    self.dirty = true;
                    self.persist();
                }
                self.screen = Screen::Main;
            }
            KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q') => {
                self.screen = Screen::Main;
            }
            _ => {}
        }
    }

    fn on_edit(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            self.edit = None;
            self.screen = Screen::Main;
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.commit_edit();
            return;
        }
        if key.code == KeyCode::Tab {
            if let Some(edit) = self.edit.as_mut() {
                edit.focus = match edit.focus {
                    EditFocus::Name => EditFocus::Input,
                    EditFocus::Input => EditFocus::Expected,
                    EditFocus::Expected => EditFocus::Name,
                };
            }
            return;
        }
        if key.code == KeyCode::BackTab {
            if let Some(edit) = self.edit.as_mut() {
                edit.focus = match edit.focus {
                    EditFocus::Name => EditFocus::Expected,
                    EditFocus::Input => EditFocus::Name,
                    EditFocus::Expected => EditFocus::Input,
                };
            }
            return;
        }

        let Some(edit) = self.edit.as_mut() else {
            return;
        };
        match edit.focus {
            EditFocus::Name => match key.code {
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    edit.name.push(c);
                }
                KeyCode::Backspace => {
                    edit.name.pop();
                }
                _ => {}
            },
            EditFocus::Input => {
                edit.input.input(to_input(key));
            }
            EditFocus::Expected => {
                edit.expected.input(to_input(key));
            }
        }
    }

    fn begin_edit(&mut self, is_new: bool) {
        let mut input = TextArea::default();
        input.set_placeholder_text("stdin for the program…");
        let mut expected = TextArea::default();
        expected.set_placeholder_text("leave empty to skip judging (run only)");
        let (id, name) = if is_new {
            (
                None,
                format!("case {}", self.bank.cases.len() + 1),
            )
        } else if let Some(c) = self.selected_case() {
            input = TextArea::from(c.input.lines().map(|s| s.to_string()));
            if let Some(exp) = &c.expected {
                expected = TextArea::from(exp.lines().map(|s| s.to_string()));
            }
            (Some(c.id), c.name.clone())
        } else {
            return;
        };
        style_textarea(&mut input);
        style_textarea(&mut expected);
        self.edit = Some(EditState {
            is_new,
            id,
            name,
            input,
            expected,
            focus: if is_new { EditFocus::Name } else { EditFocus::Input },
        });
        self.screen = Screen::Edit;
    }

    fn commit_edit(&mut self) {
        let Some(edit) = self.edit.take() else {
            return;
        };
        let name = {
            let n = edit.name.trim();
            if n.is_empty() {
                "untitled".to_string()
            } else {
                n.to_string()
            }
        };
        let input = edit.input.lines().join("\n");
        let exp_raw = edit.expected.lines().join("\n");
        let expected = if exp_raw.trim().is_empty() {
            None
        } else {
            Some(exp_raw)
        };
        if edit.is_new {
            let id = self.bank.add(name, input, expected);
            self.list_state.select(self.bank.index_of(id));
        } else if let Some(id) = edit.id {
            if let Some(c) = self.bank.get_mut(id) {
                c.name = name;
                c.input = input;
                c.expected = expected;
                c.last = None;
            }
        }
        self.dirty = true;
        self.persist();
        self.screen = Screen::Main;
    }

    fn move_sel(&mut self, delta: i32) {
        let n = self.bank.cases.len();
        if n == 0 {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0) as i32;
        let next = (i + delta).rem_euclid(n as i32) as usize;
        self.list_state.select(Some(next));
        self.detail_scroll = 0;
    }

    fn run_selected(&mut self) {
        let Some(id) = self.selected_id() else {
            return;
        };
        self.spawn_run(vec![id]);
    }

    fn run_all(&mut self) {
        let ids: Vec<u32> = self.bank.cases.iter().map(|c| c.id).collect();
        if ids.is_empty() {
            self.toast("no test cases");
            return;
        }
        self.spawn_run(ids);
    }

    fn spawn_run(&mut self, ids: Vec<u32>) {
        if self.busy {
            return;
        }
        let Some(cpp) = self.cpp.clone() else {
            return;
        };
        if self.compiler.contains(' ') && !Path::new(&self.compiler).exists() {
            self.toast(self.compiler.clone());
            return;
        }
        let cases: Vec<(u32, String, Option<String>)> = ids
            .iter()
            .filter_map(|id| {
                self.bank.cases.iter().find(|c| c.id == *id).map(|c| {
                    (c.id, c.input.clone(), c.expected.clone())
                })
            })
            .collect();
        if cases.is_empty() {
            return;
        }
        self.busy = true;
        self.status = "compiling…".into();
        let compiler = self.compiler.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            match runner::compile(&compiler, &cpp) {
                Ok(out) => {
                    let total = cases.len();
                    let _ = tx.send(WorkerEvent::CompileOk {
                        duration_ns: out.duration_ns,
                        total,
                    });
                    for (i, (id, input, expected)) in cases.into_iter().enumerate() {
                        let record =
                            runner::run_case(&out.binary, &input, expected.as_deref());
                        let _ = tx.send(WorkerEvent::CaseDone {
                            id,
                            record,
                            done: i + 1,
                            total,
                        });
                    }
                    let _ = fs::remove_file(&out.binary);
                    let _ = tx.send(WorkerEvent::BatchDone);
                }
                Err(e) => {
                    let _ = tx.send(WorkerEvent::CompileFail(format!("{e:#}")));
                }
            }
        });
    }

    pub fn drain_worker(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                WorkerEvent::CompileOk { duration_ns, total } => {
                    self.status = format!(
                        "compiled in {}  ·  running 0/{total}",
                        store::format_duration(duration_ns)
                    );
                }
                WorkerEvent::CompileFail(msg) => {
                    self.busy = false;
                    self.toast(format!("compile error\n\n{msg}"));
                }
                WorkerEvent::CaseDone {
                    id,
                    record,
                    done,
                    total,
                } => {
                    if let Some(c) = self.bank.get_mut(id) {
                        c.last = Some(record);
                    }
                    self.status = format!("running {done}/{total}");
                    self.dirty = true;
                }
                WorkerEvent::BatchDone => {
                    self.busy = false;
                    self.persist();
                    let (ac, wa, tle, re, ok) = tally(&self.bank);
                    self.status = format!(
                        "done  ·  AC {ac}  WA {wa}  TLE {tle}  RE {re}  OK {ok}"
                    );
                }
            }
        }
    }
}

fn tally(bank: &Bank) -> (usize, usize, usize, usize, usize) {
    let mut ac = 0;
    let mut wa = 0;
    let mut tle = 0;
    let mut re = 0;
    let mut ok = 0;
    for c in &bank.cases {
        match c.last.as_ref().map(|r| r.verdict) {
            Some(Verdict::Pass) => ac += 1,
            Some(Verdict::Fail) => wa += 1,
            Some(Verdict::Tle) => tle += 1,
            Some(Verdict::Runtime) => re += 1,
            Some(Verdict::Ran) => ok += 1,
            None => {}
        }
    }
    (ac, wa, tle, re, ok)
}

fn scan_cpp() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(rd) = fs::read_dir(&cwd) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("cpp") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn style_textarea(t: &mut TextArea<'_>) {
    t.set_cursor_line_style(ratatui::style::Style::default());
}

fn to_input(key: KeyEvent) -> Input {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let k = match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter => Key::Enter,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Delete => Key::Delete,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab => Key::Tab,
        _ => Key::Null,
    };
    Input { key: k, ctrl, alt, shift }
}
