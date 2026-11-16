use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::{self, BufReader, Stdout, Write};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use serde::Deserialize;
use shared_contract::{JfpMessage, JfpReader, JFP_VERSION};

type Backend = CrosstermBackend<Stdout>;

// --- Presets ---

#[derive(Deserialize, Clone)]
struct PresetEntry {
    name: String,
    description: String,
    #[serde(default)]
    slots: Vec<String>,
}

#[derive(Deserialize)]
struct PresetsFile {
    preset: Vec<PresetEntry>,
}

fn presets_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XCRATESX_CONFIG_DIR") {
        return PathBuf::from(dir).join("presets.toml");
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("../../presets.toml");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from("presets.toml")
}

fn load_presets() -> Vec<PresetEntry> {
    let path = presets_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(file) = toml::from_str::<PresetsFile>(&content) {
            if !file.preset.is_empty() {
                return file.preset;
            }
        }
    }
    vec![PresetEntry {
        name: "Everything".into(),
        description: "All installed plugins".into(),
        slots: vec![],
    }]
}

fn plugins_for_preset(preset: &PresetEntry) -> Vec<PluginSpec> {
    let all = load_plugins();
    if preset.slots.is_empty() {
        return all;
    }
    all.into_iter()
        .filter(|p| preset.slots.contains(&p.binary))
        .collect()
}

enum RunResult {
    Quit,
    BackToPresets,
}

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

fn is_safe_binary_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && Path::new(name).is_relative()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn parse_plugins(content: &str) -> Result<Vec<PluginSpec>, String> {
    let config: ConfigFile =
        toml::from_str(content).map_err(|e| format!("Invalid workspace.toml: {e}"))?;
    let mut keys = HashSet::new();
    let mut binaries = HashSet::new();
    let mut plugins = Vec::new();
    for entry in config.slot {
        let key = entry.key.chars().next().ok_or("Empty slot shortcut")?;
        if !is_safe_binary_name(&entry.binary)
            || entry.key.chars().count() != 1
            || "qprh".contains(key)
            || !key.is_ascii_alphanumeric()
            || !keys.insert(key)
            || !binaries.insert(entry.binary.clone())
        {
            return Err(format!(
                "Invalid or duplicate slot: {} ({})",
                entry.label, entry.key
            ));
        }
        plugins.push(PluginSpec {
            binary: entry.binary,
            label: entry.label,
            key,
            description: entry.description,
        });
    }
    if plugins.is_empty() {
        return Err("workspace.toml has no slots".into());
    }
    Ok(plugins)
}
fn load_plugins_checked() -> Result<Vec<PluginSpec>, String> {
    let path = config_path();
    let content = fs::read_to_string(&path).map_err(|e| {
        format!(
            "{}: {e}. Set XCRATESX_CONFIG_DIR if needed.",
            path.display()
        )
    })?;
    parse_plugins(&content)
}
fn load_plugins() -> Vec<PluginSpec> {
    load_plugins_checked().unwrap_or_default()
}
fn validate_configuration() -> Result<(), String> {
    let plugins = load_plugins_checked()?;
    let raw = fs::read_to_string(presets_path()).map_err(|e| format!("presets.toml: {e}"))?;
    let presets: PresetsFile = toml::from_str(&raw).map_err(|e| format!("presets.toml: {e}"))?;
    if presets.preset.is_empty() {
        return Err("No presets configured".into());
    }
    for preset in presets.preset {
        for slot in preset.slots {
            if !plugins.iter().any(|p| p.binary == slot) {
                return Err(format!(
                    "Preset {} references unknown plugin {slot}",
                    preset.name
                ));
            }
        }
    }
    Ok(())
}

fn config_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XCRATESX_CONFIG_DIR") {
        return PathBuf::from(dir).join("workspace.toml");
    }
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

fn read_frames(stdout: ChildStdout, sender: SyncSender<JfpMessage>) {
    let mut reader = JfpReader::new(BufReader::new(stdout));

    while let Some(message) = reader.next_frame() {
        if sender.send(message).is_err() {
            return;
        }
    }

    if reader.is_incomplete() {
        let _ = sender.send(JfpMessage::reply(
            "CRASH",
            "Plugin exited with an incomplete JFP frame.",
        ));
    } else {
        let _ = sender.send(JfpMessage::reply(
            "CRASH",
            "Plugin closed stdout. See plugin stderr log.",
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
        let log_dir = std::env::var_os("XCRATESX_LOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("XDG_STATE_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                            .join(".local/state")
                    })
                    .join("xcratesx")
            });
        fs::create_dir_all(&log_dir)?;
        let name = executable
            .file_name()
            .ok_or_else(|| io::Error::other("missing plugin name"))?;
        let log = fs::File::create(log_dir.join(format!("{}.stderr.log", name.to_string_lossy())))?;
        let mut child = Command::new(executable)
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log)
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("missing stdin"))?;
        // A plugin that stops reading must not freeze the terminal event loop.
        let fd = stdin.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
        {
            let error = io::Error::last_os_error();
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing stdout"))?;

        let (sender, output) = mpsc::sync_channel(64);

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
        self.output.try_recv().ok()
    }

    fn shutdown(&mut self) {
        if self.reader.is_none() {
            return;
        }
        // Each plugin starts a separate process group. Include Cargo descendants.
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();

        // Disconnect a reader blocked on a full queue before joining it.
        self.output = mpsc::channel().1;

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

enum KeyAction {
    Continue,
    Quit,
    Presets,
}

struct Engine {
    local_response: Option<JfpMessage>,
    preset_slots: Vec<String>,
    history: VecDeque<JfpMessage>,
    show_history: bool,
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
            local_response: Some(JfpMessage::reply("OK", crate_workbench::experiments::HELP)),
            preset_slots: Vec::new(),
            history: VecDeque::new(),
            show_history: false,
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
                self.fault = Some(format!("cannot start {}: {err}", path.display()));
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

    fn reload_config(&mut self) {
        self.local_response = None;
        let loaded = match load_plugins_checked() {
            Ok(plugins) => plugins,
            Err(error) => {
                self.local_response = Some(JfpMessage::reply("ERROR", error));
                return;
            }
        };
        let new_plugins = loaded
            .into_iter()
            .filter(|p| self.preset_slots.is_empty() || self.preset_slots.contains(&p.binary))
            .collect::<Vec<_>>();

        if new_plugins.is_empty() {
            self.fault = Some("Reload: no slots in workspace.toml".into());
            return;
        }

        let current_binary = self.active().binary.clone();
        self.stop_plugin();
        self.plugins = new_plugins;

        self.active_index = self
            .plugins
            .iter()
            .position(|p| p.binary == current_binary)
            .unwrap_or(0);

        self.message = Some(JfpMessage::reply(
            "OK",
            format!(
                "CONFIG RELOAD | Loaded {} slots from workspace.toml",
                self.plugins.len()
            ),
        ));
        self.tick_count = 0;
        self.frames = 0;
        self.scroll_offset = 0;
        self.start_plugin();
    }

    fn switch_by_key(&mut self, key: char) {
        self.local_response = None;
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
            while let Some(message) = process.latest() {
                if self.history.back() != Some(&message) {
                    if self.history.len() == 32 {
                        self.history.pop_front();
                    }
                    self.history.push_back(message.clone());
                }
                self.message = Some(message);
                self.frames = self.frames.wrapping_add(1);
            }
        }
    }

    fn submit_command(&mut self) {
        let body = std::mem::take(&mut self.input_buffer);
        self.local_response = None;
        if body.trim().starts_with(':') {
            self.experiment_command(body.trim());
            return;
        }
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

    fn experiment_command(&mut self, command: &str) {
        if command == ":build" || command == ":graph" {
            let (binary, body) = if command == ":build" {
                ("plugin_crate_assembler", "test:@saved")
            } else {
                ("plugin_crate_analyzer", "analyze @saved")
            };
            if !self.plugins.iter().any(|p| p.binary == binary) {
                if let Some(plugin) = load_plugins().into_iter().find(|p| p.binary == binary) {
                    self.plugins.push(plugin);
                }
            }
            if let Some(key) = self
                .plugins
                .iter()
                .find(|p| p.binary == binary)
                .map(|p| p.key)
            {
                self.switch_by_key(key);
                self.input_buffer = body.into();
                self.submit_command();
            } else {
                self.local_response = Some(JfpMessage::reply(
                    "ERROR",
                    "Required workbench plugin is missing from workspace.toml.",
                ));
            }
        } else {
            let result = crate_workbench::experiments::Store::default_location()
                .and_then(|store| store.command(command));
            self.local_response = Some(match result {
                Ok(body) => JfpMessage::reply("OK", body),
                Err(error) => JfpMessage::reply("ERROR", error),
            });
            self.scroll_offset = 0;
        }
    }

    fn handle_key(&mut self, code: KeyCode) -> KeyAction {
        match code {
            KeyCode::Tab => {
                self.command_mode = !self.command_mode;
                KeyAction::Continue
            }
            KeyCode::Enter => {
                if self.command_mode {
                    self.submit_command();
                    self.command_mode = false;
                }
                KeyAction::Continue
            }
            KeyCode::Backspace => {
                if self.command_mode {
                    self.input_buffer.pop();
                }
                KeyAction::Continue
            }
            KeyCode::Esc => {
                if self.command_mode {
                    self.command_mode = false;
                    KeyAction::Continue
                } else {
                    KeyAction::Quit
                }
            }
            KeyCode::Up if !self.command_mode => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                KeyAction::Continue
            }
            KeyCode::Down if !self.command_mode => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                KeyAction::Continue
            }
            KeyCode::PageUp if !self.command_mode => {
                self.scroll_offset = self
                    .scroll_offset
                    .saturating_sub(self.body_height.saturating_sub(2));
                KeyAction::Continue
            }
            KeyCode::PageDown if !self.command_mode => {
                self.scroll_offset = self
                    .scroll_offset
                    .saturating_add(self.body_height.saturating_sub(2));
                KeyAction::Continue
            }
            KeyCode::Home if !self.command_mode => {
                self.scroll_offset = 0;
                KeyAction::Continue
            }
            KeyCode::Char('q') if !self.command_mode => KeyAction::Quit,
            KeyCode::Char('p') if !self.command_mode => KeyAction::Presets,
            KeyCode::Char('r') if !self.command_mode => {
                self.reload_config();
                KeyAction::Continue
            }
            KeyCode::Char('h') if !self.command_mode => {
                self.local_response = None;
                self.show_history = !self.show_history;
                self.scroll_offset = 0;
                KeyAction::Continue
            }
            KeyCode::Char(character) if self.command_mode => {
                if !character.is_control() && self.input_buffer.len() < 2048 {
                    self.input_buffer.push(character);
                }
                KeyAction::Continue
            }
            _ => {
                if !self.command_mode {
                    if let KeyCode::Char(ch) = code {
                        self.switch_by_key(ch);
                    }
                }
                KeyAction::Continue
            }
        }
    }

    fn crashed(&self) -> bool {
        self.message
            .as_ref()
            .is_some_and(|message| message.status == "CRASH")
    }

    fn status(&self) -> &str {
        if let Some(reply) = &self.local_response {
            return &reply.status;
        }
        if self.fault.is_some() {
            return "ERROR";
        }

        self.message
            .as_ref()
            .map(|message| message.status.as_str())
            .unwrap_or("WAIT")
    }

    fn body(&self) -> String {
        if let Some(reply) = &self.local_response {
            return reply.body.clone();
        }
        if self.show_history {
            return self
                .history
                .iter()
                .map(|m| format!("[{}]\n{}", m.status, m.body))
                .collect::<Vec<_>>()
                .join("\n\n");
        }
        if let Some(fault) = &self.fault {
            return format!("{}: ERROR. {fault}", self.active().label);
        }

        match &self.message {
            Some(message) => message.body.clone(),
            None => String::from("Waiting for plugin response..."),
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
            "{} | {} | tick:{} rx:{} pid:{} | {} | p:presets r:reload h:history q:quit",
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
            "XcratesX :: {} :: {} :: {JFP_VERSION}{scroll_hint}",
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
        (" COMMAND :: ENTER send :: TAB navigate ", Color::Cyan)
    } else {
        (
            " COMMAND :: TAB to type :: :help experiments ",
            Color::DarkGray,
        )
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

fn run(terminal: &mut Terminal<Backend>, engine: &mut Engine) -> io::Result<RunResult> {
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

            match engine.handle_key(key.code) {
                KeyAction::Continue => {}
                KeyAction::Quit => return Ok(RunResult::Quit),
                KeyAction::Presets => return Ok(RunResult::BackToPresets),
            }
        }
    }
}

// --- Preset selection screen ---

fn render_presets(frame: &mut Frame, presets: &[PresetEntry], selected: usize) {
    let area = frame.area();

    let block = Block::default()
        .title(format!(
            " XcratesX — Choose your workspace :: {JFP_VERSION} "
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [body_area, footer_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    let mut lines: Vec<Line> = vec![Line::from(""), Line::from("")];

    for (i, preset) in presets.iter().enumerate() {
        let marker = if i == selected { "▸ " } else { "  " };
        let style = if i == selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::styled(
            format!(
                "{marker}{}. {} — {}",
                i + 1,
                preset.name,
                preset.description
            ),
            style,
        ));
        lines.push(Line::from(""));
    }

    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Left), body_area);

    frame.render_widget(
        Paragraph::new(Line::from("↑↓ navigate | Enter select | q quit"))
            .style(Style::default().fg(Color::DarkGray)),
        footer_area,
    );
}

fn run_preset_select(
    terminal: &mut Terminal<Backend>,
    presets: &[PresetEntry],
) -> io::Result<Option<usize>> {
    let mut selected: usize = 0;

    loop {
        terminal.draw(|frame| render_presets(frame, presets, selected))?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Up => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down => {
                    if selected + 1 < presets.len() {
                        selected += 1;
                    }
                }
                KeyCode::Enter => {
                    return Ok(Some(selected));
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    return Ok(None);
                }
                KeyCode::Char(ch) => {
                    if let Some(n) = ch.to_digit(10) {
                        let idx = n as usize;
                        if idx >= 1 && idx <= presets.len() {
                            return Ok(Some(idx - 1));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

struct TerminalCleanup;
impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

fn main() -> io::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if !args.is_empty() {
        if args == ["--help"] || args == ["-h"] {
            println!("XcratesX\n\nRun without arguments for the terminal UI.\n--experiment ':COMMAND' runs a saved-experiment command without a terminal.\n\n{}", crate_workbench::experiments::HELP);
            return Ok(());
        }
        if args == ["--check-config"] {
            validate_configuration().map_err(io::Error::other)?;
            for plugin in load_plugins_checked().map_err(io::Error::other)? {
                if !target_dir().join(&plugin.binary).is_file() {
                    return Err(io::Error::other(format!(
                        "Missing {}. Run cargo build --workspace.",
                        plugin.binary
                    )));
                }
            }
            println!("Configuration and plugin binaries: OK");
            return Ok(());
        }
        if args.len() == 2 && args[0] == "--experiment" {
            let result =
                crate_workbench::experiments::command(&args[1]).map_err(io::Error::other)?;
            println!("{result}");
            return Ok(());
        }
        return Err(io::Error::other("Unknown arguments. Use --help."));
    }
    validate_configuration().map_err(io::Error::other)?;
    let presets = load_presets();
    let _cleanup = TerminalCleanup;
    let mut terminal = init_terminal()?;

    loop {
        let selected = run_preset_select(&mut terminal, &presets)?;

        let Some(idx) = selected else {
            restore_terminal(&mut terminal)?;
            return Ok(());
        };

        let plugins = plugins_for_preset(&presets[idx]);

        if plugins.is_empty() {
            continue;
        }

        let mut engine = Engine::new(plugins);
        engine.preset_slots = presets[idx].slots.clone();
        let result = run(&mut terminal, &mut engine)?;
        engine.stop_plugin();

        match result {
            RunResult::Quit => {
                restore_terminal(&mut terminal)?;
                return Ok(());
            }
            RunResult::BackToPresets => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_multichar_and_reserved_slots() {
        for key in ["10", "q", "", "/"] {
            let input = format!("[[slot]]\nkey = {key:?}\nlabel = 'test'\nbinary = 'plugin_test'");
            assert!(parse_plugins(&input).is_err());
        }
        let slot = "[[slot]]\nkey = '1'\nlabel = 'test'\nbinary = 'plugin_test'\n";
        assert!(parse_plugins(&format!("{slot}{slot}")).is_err());
        assert!(parse_plugins("slot = []").is_err());
    }

    #[test]
    fn shipped_slots_are_unique_and_built() {
        let config: ConfigFile = toml::from_str(include_str!("../../workspace.toml")).unwrap();
        let workspace: toml::Value = toml::from_str(include_str!("../../Cargo.toml")).unwrap();
        let members = workspace["workspace"]["members"].as_array().unwrap();
        let mut keys = HashSet::new();
        let mut binaries = HashSet::new();
        for slot in config.slot {
            assert_eq!(slot.key.chars().count(), 1);
            assert!(!"qprh".contains(&slot.key));
            assert!(keys.insert(slot.key));
            assert!(binaries.insert(slot.binary.clone()));
            assert!(members.iter().any(|m| m.as_str() == Some(&slot.binary)));
        }
        let presets: PresetsFile = toml::from_str(include_str!("../../presets.toml")).unwrap();
        for preset in presets.preset {
            assert!(preset.slots.iter().all(|s| binaries.contains(s)));
        }
    }

    #[test]
    fn process_round_trip_and_shutdown_with_full_queue() {
        // cat echoes valid requests, exercising actual pipes and frame parsing.
        let mut child = Command::new("cat")
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, output) = mpsc::sync_channel(1);
        let reader = thread::spawn(move || read_frames(stdout, tx));
        let mut process = PluginProcess {
            child,
            stdin,
            output,
            reader: Some(reader),
        };
        process
            .send_frame(&JfpMessage::request("COMMAND", "hello"))
            .unwrap();
        assert_eq!(
            process
                .output
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .body,
            "hello"
        );
        for _ in 0..5 {
            process
                .send_frame(&JfpMessage::request("TICK", "1"))
                .unwrap();
        }
        process.shutdown();
        assert!(process.child.try_wait().unwrap().is_some());
    }

    #[test]
    fn test_is_safe_binary_name() {
        assert!(is_safe_binary_name("plugin_osint"));
        assert!(is_safe_binary_name("plugin_net-diag_1"));
        assert!(!is_safe_binary_name("/bin/sh"));
        assert!(!is_safe_binary_name("../plugin_osint"));
        assert!(!is_safe_binary_name("foo/bar"));
        assert!(!is_safe_binary_name("foo\\bar"));
        assert!(!is_safe_binary_name(""));
        assert!(!is_safe_binary_name("plugin;rm"));
    }
}
