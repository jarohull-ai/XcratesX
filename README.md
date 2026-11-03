# XcratesX

Silnik TUI (ratatui/crossterm) sterujący zestawem wtyczek domenowych przez protokół **JFP v5.4.0** po stdio.

## Architektura

```
host_engine ──stdio──► plugin_osint
             ──stdio──► plugin_music
             ──stdio──► plugin_writing
             ──stdio──► plugin_audio
             ──stdio──► plugin_crucible
```

- **shared_contract** — biblioteka: `JfpMessage` (parse/serialize), `JfpReader` (streaming po BufRead)
- **host_engine** — TUI: spawnuje procesy wtyczek, wysyła ramki TICK/COMMAND, renderuje odpowiedzi
- **plugin_osint** — symulacja skanera OSINT
- **plugin_music** — odtwarzacz muzyki (czas, BPM, FFT)
- **plugin_writing** — generator tekstu (rozdziały)
- **plugin_audio** — DSP studio (obciążenie CPU, latencja)
- **plugin_crucible** — stress-test: celowy crash po 6 tickach, niepełna ramka + duży payload

## Protokół JFP

Ramka tekstowa: `[JFP_HEADER]` → nagłówki (VERSION, STATUS/INTENT) → `[END_HEADER]` → `[MESSAGE_BODY]` → treść → `[END_MESSAGE]`.

## Uruchomienie

```sh
cargo build
./target/debug/host_engine
```

Klawisze: `1`–`5` przełączają wtyczki, `Tab` tryb komend, `6` rapid-fire test, `q`/`Esc` wyjście.
