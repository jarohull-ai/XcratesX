use std::fmt::Write as _;
use std::io::{self, Write};

use shared_contract::{JfpMessage, JfpReader};
use sysinfo::{Disks, Networks, System};

struct State {
    system: System,
    last_output: String,
    initialized: bool,
}

impl State {
    fn new() -> Self {
        Self {
            system: System::new_all(),
            last_output: String::new(),
            initialized: false,
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn overview(sys: &System) -> String {
    let mut out = String::from("SYSINFO | Przegląd systemu\n\n");

    let os_name = System::name().unwrap_or_default();
    let os_ver = System::os_version().unwrap_or_default();
    let host = System::host_name().unwrap_or_default();

    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "?".into());
    let cores = sys.cpus().len();

    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    let mem_pct = if total_mem > 0 {
        used_mem as f64 / total_mem as f64 * 100.0
    } else {
        0.0
    };

    let total_swap = sys.total_swap();
    let used_swap = sys.used_swap();

    let disks = Disks::new_with_refreshed_list();
    let networks = Networks::new_with_refreshed_list();

    let _ = writeln!(out, "  System:   {os_name} {os_ver}");
    let _ = writeln!(out, "  Host:     {host}");
    let _ = writeln!(out, "  CPU:      {cpu_name} ({cores} rdzeni)");
    let _ = writeln!(
        out,
        "  RAM:      {} / {} ({mem_pct:.1}%)",
        format_bytes(used_mem),
        format_bytes(total_mem)
    );
    let _ = writeln!(
        out,
        "  Swap:     {} / {}",
        format_bytes(used_swap),
        format_bytes(total_swap)
    );
    let _ = writeln!(out, "  Dyski:    {} zamontowanych", disks.list().len());
    let _ = writeln!(out, "  Sieć:     {} interfejsów", networks.list().len());
    let _ = writeln!(
        out,
        "\n  Komendy: cpu | mem | disk | net | proc <nazwa> | all | help"
    );

    out
}

fn cpu_report(sys: &System) -> String {
    let mut out = String::from("SYSINFO | CPU\n\n");

    let name = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "?".into());
    let cores = sys.cpus().len();

    let _ = writeln!(out, "  Model:    {name}");
    let _ = writeln!(out, "  Rdzenie:  {cores}");

    if let Some(cpu) = sys.cpus().first() {
        let _ = writeln!(out, "  Czest.:   {} MHz", cpu.frequency());
    }

    let _ = writeln!(out);
    for (i, cpu) in sys.cpus().iter().enumerate() {
        let _ = writeln!(out, "  Rdzen {:>2}: {:>5.1}%", i, cpu.cpu_usage());
    }

    out
}

fn mem_report(sys: &System) -> String {
    let mut out = String::from("SYSINFO | Pamięć\n\n");

    let total = sys.total_memory();
    let used = sys.used_memory();
    let avail = sys.available_memory();
    let pct = if total > 0 {
        used as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    let _ = writeln!(
        out,
        "  RAM:      {} / {} ({pct:.1}%)",
        format_bytes(used),
        format_bytes(total)
    );
    let _ = writeln!(out, "  Dostępna: {}", format_bytes(avail));
    let _ = writeln!(
        out,
        "  Swap:     {} / {}",
        format_bytes(sys.used_swap()),
        format_bytes(sys.total_swap())
    );

    out
}

fn disk_report() -> String {
    let mut out = String::from("SYSINFO | Dyski\n\n");
    let disks = Disks::new_with_refreshed_list();

    for disk in disks.list() {
        let total = disk.total_space();
        let avail = disk.available_space();
        let used = total.saturating_sub(avail);
        let pct = if total > 0 {
            used as f64 / total as f64 * 100.0
        } else {
            0.0
        };

        let _ = writeln!(
            out,
            "  {} ({})",
            disk.mount_point().display(),
            String::from_utf8_lossy(disk.file_system().as_encoded_bytes())
        );
        let _ = writeln!(
            out,
            "    {} / {} ({pct:.1}% użyte)\n",
            format_bytes(used),
            format_bytes(total)
        );
    }

    out
}

fn net_report() -> String {
    let mut out = String::from("SYSINFO | Sieć\n\n");
    let networks = Networks::new_with_refreshed_list();

    for (name, data) in networks.list() {
        let _ = writeln!(
            out,
            "  {name}: rx {} / tx {}",
            format_bytes(data.total_received()),
            format_bytes(data.total_transmitted())
        );
    }

    if networks.list().is_empty() {
        out.push_str("  (brak interfejsów)\n");
    }

    out
}

fn proc_report(sys: &System, name: &str) -> String {
    let mut out = format!("SYSINFO | Procesy: \"{name}\"\n\n");

    let mut found = 0u32;
    for (pid, proc_info) in sys.processes() {
        if proc_info
            .name()
            .to_string_lossy()
            .to_lowercase()
            .contains(&name.to_lowercase())
        {
            let _ = writeln!(
                out,
                "  PID {:>6} | {} | CPU: {:.1}% | RAM: {}",
                pid.as_u32(),
                proc_info.name().to_string_lossy(),
                proc_info.cpu_usage(),
                format_bytes(proc_info.memory())
            );
            found += 1;
        }
    }

    if found == 0 {
        let _ = writeln!(out, "  Nie znaleziono procesów pasujących do \"{name}\"");
    } else {
        let _ = writeln!(out, "\n  Znaleziono: {found}");
    }

    out
}

fn help_text() -> String {
    String::from(
        "SYSINFO | Dostępne komendy\n\n\
         \x20 all          Pełny przegląd systemu\n\
         \x20 cpu          Informacje o procesorze\n\
         \x20 mem          Pamięć RAM i swap\n\
         \x20 disk         Zamontowane dyski\n\
         \x20 net          Interfejsy sieciowe\n\
         \x20 proc <nazwa> Szukaj procesów po nazwie\n\
         \x20 help         Ta lista komend",
    )
}

fn handle(frame: &JfpMessage, state: &mut State) -> Option<JfpMessage> {
    match frame.intent.as_deref()? {
        "COMMAND" => {
            state.system.refresh_all();

            let input = frame.body.trim();
            let report = match input.split_once(' ') {
                Some(("proc", name)) if !name.trim().is_empty() => {
                    proc_report(&state.system, name.trim())
                }
                _ => match input.to_lowercase().as_str() {
                    "" | "all" => overview(&state.system),
                    "cpu" => cpu_report(&state.system),
                    "mem" => mem_report(&state.system),
                    "disk" => disk_report(),
                    "net" => net_report(),
                    "help" => help_text(),
                    other => {
                        if other.starts_with("proc") {
                            String::from("SYSINFO | Użycie: proc <nazwa_procesu>")
                        } else {
                            format!("SYSINFO | Nieznana komenda: \"{other}\". Wpisz \"help\".")
                        }
                    }
                },
            };

            state.last_output = report.clone();
            Some(JfpMessage::reply("OK", report))
        }
        "TICK" => {
            if !state.initialized {
                state.system.refresh_all();
                state.last_output = overview(&state.system);
                state.initialized = true;
            }
            Some(JfpMessage::reply("OK", &state.last_output))
        }
        _ => None,
    }
}

fn main() {
    let stdin = io::stdin();
    let mut reader = JfpReader::new(stdin.lock());
    let mut stdout = io::stdout();
    let mut state = State::new();

    while let Some(frame) = reader.next_frame() {
        let Some(message) = handle(&frame, &mut state) else {
            continue;
        };

        if writeln!(stdout, "{}", message.to_jfp_string()).is_err() {
            break;
        }

        if stdout.flush().is_err() {
            break;
        }
    }
}
