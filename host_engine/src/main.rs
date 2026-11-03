use std::fs;
use std::io::{self, BufReader, Stdout, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use serde::Deserialize;
use shared_contract::{JfpMessage, JfpReader, JFP_VERSION};

type Backend = CrosstermBackend<Stdout>;

#[derive(Clone, PartialEq, Eq)]
struct PluginSpec {
    binary: String,
    label: String,
    key: char,
    description: String,
}

#[derive(Deserialize)]
struct ConfigFile {
    slot: Vec<SlotEntry>,
}

#[derive(Deserialize)]
struct SlotEntry {
    key: String,
    label: String,
    binary: String,
    #[serde(default)]
    description: String,
}

fn load_plugins() -> Vec<PluginSpec> {
    let config_path = config_path();

    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(config) = toml::from_str::<ConfigFile>(&content) {
            let plugins: Vec<PluginSpec> = config
                .slot
                .into_iter()
                .filter_map(|entry| {
                    let key = entry.key.chars().next()?;
                    Some(PluginSpec {
                        binary: entry.binary,
                        label: entry.label,
                        key,
                        description: entry.description,
                    })
                })
                .collect();

            if !plugins.is_empty() {
                return plugins;
            }
        }
    }

    default_plugins()
}

fn default_plugins() -> Vec<PluginSpec> {
    vec![
        PluginSpec {
            binary: "plugin_osint".into(),
            label: "OSINT".into(),
            key: '1',
            description: String::new(),
        },
        PluginSpec {
            binary: "plugin_search".into(),
            label: "SEARCH".into(),
            key: '2',
            description: String::new(),
        },
        PluginSpec {
            binary: "plugin_audio".into(),
            label: "AUDIO".into(),
            key: '3',
            description: String::new(),
        },
        PluginSpec {
            binary: "plugin_writing".into(),
            label: "WRITING".into(),
            key: '4',
            description: String::new(),
        },
        PluginSpec {
            binary: "plugin_music".into(),
            label: "MUSIC".into(),
            key: '5',
            description: String::new(),
        },
        PluginSpec {
            binary: "plugin_crucible".into(),
            label: "CRUCIBLE".into(),
            key: '9',
            description: String::new(),
        },
    ]
}

fn config_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("../../workspace.toml");
            if candidate.exists() {
                return candidate;
            }
        }
    }

    PathBuf::from("workspace.toml")
}

fn read_frames(stdout: ChildStdout, sender: Sender<JfpMessage>) {
    let mut reader = JfpReader::new(BufReader::new(stdout));

    while let Some(message) = reader.next_frame() {
        if sender.send(message).is_err() {
            return;
        }
    }

    if reader.is_incomplete() {
        let _ = sender.send(JfpMessage::reply(
            "CRASH",
            "Proces potomny zgasl niespodziewanie (EOF) z niedomknieta ramka JFP.",
        ));
    }
}

struct PluginProcess {
    child: Child,
    stdin: ChildStdin,
    output: Receiver<JfpMessage>,
    reader: Option<JoinHandle<()>>,
}

impl PluginProcess {
    fn spawn(executable: &Path) -> io::Result<Self> {
        let mut child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("brak stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("brak stdout"))?;

        let (sender, output) = mpsc::channel();

        let reader = thread::spawn(move || read_frames(stdout, sender));

        Ok(Self {
            child,
            stdin,
            output,
            reader: Some(reader),
        })
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn send_frame(&mut self, frame: &JfpMessage) -> io::Result<()> {
        writeln!(self.stdin, "{}", frame.to_jfp_string())?;
        self.stdin.flush()
    }

    fn latest(&self) -> Option<JfpMessage> {
        let mut latest = None;

        while let Ok(message) = self.output.try_recv() {
            latest = Some(message);
        }

        latest
    }

    fn shutdown(&mut self) {
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();

        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct Engine {
    target: PathBuf,
    plugins: Vec<PluginSpec>,
    active_index: usize,
    process: Option<PluginProcess>,
    message: Option<JfpMessage>,
    fault: Option<String>,
    tick_count: u64,
    frames: u64,
    input_buffer: String,
    command_mode: bool,
    hold_ticks: u8,
    scroll_offset: u16,
    body_height: u16,
}

impl Engine {
    fn new(plugins: Vec<PluginSpec>) -> Self {
        let mut engine = Self {
            target: target_dir(),
            active_index: 0,
            plugins,
            process: None,
            message: None,
            fault: None,
            tick_count: 0,
            frames: 0,
            input_buffer: String::new(),
            command_mode: false,
            hold_ticks: 0,
            scroll_offset: 0,
            body_height: 24,
        };

        engine.start_plugin();
        engine
    }

    fn active(&self) -> &PluginSpec {
        &self.plugins[self.active_index]
    }

    fn start_plugin(&mut self) {
        let path = self.target.join(&self.active().binary);

        match PluginProcess::spawn(&path) {
            Ok(process) => {
                self.process = Some(process);
                self.fault = None;
            }
            Err(err) => {
                self.process = None;
                self.fault = Some(format!("nie mozna uruchomic {}: {err}", path.display()));
            }
        }
    }

    fn stop_plugin(&mut self) {
        if let Some(mut process) = self.process.take() {
            process.shutdown();

            if let Some(message) = process.latest() {
                if message.status == "CRASH" {
                    self.message = Some(message);
                    self.fault = None;
                }
            }
        }
    }

    fn switch_by_key(&mut self, key: char) {
        let Some(index) = self.plugins.iter().position(|p| p.key == key) else {
            return;
        };

        if self.process.is_some() && self.active_index == index {
            return;
        }

        self.stop_plugin();
        self.active_index = index;
        self.message = None;
        self.tick_count = 0;
        self.frames = 0;
        self.input_buffer.clear();
        self.command_mode = false;
        self.hold_ticks = 0;
        self.scroll_offset = 0;
        self.start_plugin();
    }

    fn tick(&mut self) {
        if self.hold_ticks > 0 {
            self.hold_ticks -= 1;
            self.drain();
            return;
        }

        self.tick_count = self.tick_count.wrapping_add(1);

        let frame = JfpMessage::request("TICK", self.tick_count.to_string());

        let sent = match self.process.as_mut() {
            Some(process) => process.send_frame(&frame),
            None => return,
        };

        if let Err(err) = sent {
            self.stop_plugin();

            if !self.crashed() {
                self.fault = Some(format!("stdin: {err}"));
            }

            return;
        }

        self.drain();
    }

    fn drain(&mut self) {
        if let Some(process) = self.process.as_ref() {
            if let Some(message) = process.latest() {
                self.message = Some(message);
                self.frames = self.frames.wrapping_add(1);
            }
        }
    }

    fn submit_command(&mut self) {
        let body = std::mem::take(&mut self.input_buffer);
        let frame = JfpMessage::request("COMMAND", body);

        let sent = match self.process.as_mut() {
            Some(process) => process.send_frame(&frame),
            None => return,
        };

        match sent {
            Ok(()) => self.hold_ticks = 2,
            Err(err) => {
                self.stop_plugin();

                if !self.crashed() {
                    self.fault = Some(format!("stdin: {err}"));
                }
            }
        }
    }

    fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Tab => {
                self.command_mode = !self.command_mode;
                false
            }
            KeyCode::Enter => {
                if self.command_mode {
                    self.submit_command();
                    self.command_mode = false;
                }
                false
            }
            KeyCode::Backspace => {
                if self.command_mode {
                    self.input_buffer.pop();
                }
                false
            }
            KeyCode::Esc => {
                if self.command_mode {
                    self.command_mode = false;
                    false
                } else {
                    true
                }
            }
            KeyCode::Up if !self.command_mode => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                false
            }
            KeyCode::Down if !self.command_mode => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                false
            }
            KeyCode::PageUp if !self.command_mode => {
                self.scroll_offset = self
                    .scroll_offset
                    .saturating_sub(self.body_height.saturating_sub(2));
                false
            }
            KeyCode::PageDown if !self.command_mode => {
                self.scroll_offset = self
                    .scroll_offset
                    .saturating_add(self.body_height.saturating_sub(2));
                false
            }
            KeyCode::Home if !self.command_mode => {
                self.scroll_offset = 0;
                false
            }
            KeyCode::Char('q') if !self.command_mode => true,
            KeyCode::Char(character) if self.command_mode => {
                if !character.is_control() {
                    self.input_buffer.push(character);
                }
                false
            }
            _ => {
                if !self.command_mode {
                    if let KeyCode::Char(ch) = code {
                        self.switch_by_key(ch);
                    }
                }
                false
            }
        }
    }

    fn crashed(&self) -> bool {
        self.message
            .as_ref()
            .is_some_and(|message| message.status == "CRASH")
    }

    fn status(&self) -> &str {
        if self.fault.is_some() {
            return "ERROR";
        }

        self.message
            .as_ref()
            .map(|message| message.status.as_str())
            .unwrap_or("WAIT")
    }

    fn body(&self) -> String {
        if let Some(fault) = &self.fault {
            return format!("{}: BLAD. {fault}", self.active().label);
        }

        match &self.message {
            Some(message) => message.body.clone(),
            None => String::from("(czekam na ramke JFP z procesu...)"),
        }
    }

    fn footer(&self) -> String {
        let pid = match &self.process {
            Some(process) => process.pid().to_string(),
            None => String::from("-"),
        };

        let keys: String = self
            .plugins
            .iter()
            .map(|p| format!("{}:{}", p.key, p.label.to_lowercase()))
            .collect::<Vec<_>>()
            .join(" ");

        format!(
            "{} | {} | tick:{} rx:{} pid:{} | {} | q:quit",
            self.active().label,
            self.status(),
            self.tick_count,
            self.frames,
            pid,
            keys,
        )
    }
}

fn target_dir() -> PathBuf {
    if let Ok(executable) = std::env::current_exe() {
        if let Some(dir) = executable.parent() {
            return dir.to_path_buf();
        }
    }

    PathBuf::from("target/debug")
}

fn init_terminal() -> io::Result<Terminal<Backend>> {
    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(out))
}

fn restore_terminal(terminal: &mut Terminal<Backend>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}

fn frame_color(status: &str) -> Color {
    match status {
        "OK" => Color::Green,
        "WAIT" => Color::Yellow,
        "ERROR" | "CRASH" => Color::Red,
        _ => Color::Cyan,
    }
}

fn render(frame: &mut Frame, engine: &mut Engine) {
    let [output_area, command_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(frame.area());

    let status = engine.status();
    let color = frame_color(status);

    let scroll_hint = if engine.scroll_offset > 0 {
        format!(" [scroll:{}]", engine.scroll_offset)
    } else {
        String::new()
    };

    let block = Block::default()
        .title(format!(
            "JFP SILNIK :: {} :: {} :: {JFP_VERSION}{scroll_hint}",
            engine.active().label,
            status,
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color));

    let inner = block.inner(output_area);
    frame.render_widget(block, output_area);

    let [body_area, footer_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    engine.body_height = body_area.height;

    frame.render_widget(
        Paragraph::new(engine.body())
            .wrap(Wrap { trim: false })
            .scroll((engine.scroll_offset, 0)),
        body_area,
    );

    frame.render_widget(
        Paragraph::new(Line::from(engine.footer())).style(Style::default().fg(Color::DarkGray)),
        footer_area,
    );

    let (command_title, command_color) = if engine.command_mode {
        (
            " KOMENDA :: ENTER wysyla :: TAB sterowanie ",
            Color::Cyan,
        )
    } else {
        (" KOMENDA :: TAB aby pisac ", Color::DarkGray)
    };

    let command_block = Block::default()
        .title(command_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(command_color));

    let command_inner = command_block.inner(command_area);
    frame.render_widget(command_block, command_area);

    let prompt = format!("> {}", engine.input_buffer);
    let prompt_width = Line::from(prompt.as_str()).width() as u16;
    let prompt_color = if engine.command_mode {
        Color::White
    } else {
        Color::DarkGray
    };

    frame.render_widget(
        Paragraph::new(prompt).style(Style::default().fg(prompt_color)),
        command_inner,
    );

    if engine.command_mode {
        let cursor_x = command_inner
            .x
            .saturating_add(prompt_width)
            .min(command_inner.right().saturating_sub(1));

        frame.set_cursor_position((cursor_x, command_inner.y));
    }
}

fn run(terminal: &mut Terminal<Backend>, engine: &mut Engine) -> io::Result<()> {
    loop {
        engine.tick();
        terminal.draw(|frame| render(frame, engine))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if engine.handle_key(key.code) {
                return Ok(());
            }
        }
    }
}

fn main() -> io::Result<()> {
    let plugins = load_plugins();

    if plugins.is_empty() {
        eprintln!("Brak slotow w workspace.toml i brak domyslnych wtyczek.");
        return Ok(());
    }

    let mut engine = Engine::new(plugins);
    let mut terminal = init_terminal()?;
    let result = run(&mut terminal, &mut engine);
    engine.stop_plugin();
    restore_terminal(&mut terminal)?;
    result
}
