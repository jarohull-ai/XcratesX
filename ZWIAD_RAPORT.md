# Zwiad XcratesX — raport praktyczny

Stan: 4 listopada 2026. Oparty na kodzie i konfiguracji projektu, nie na deklaracjach README.

## Wniosek wprost

XcratesX jest działającym szkieletem **terminalowego workbencha dla kilku małych narzędzi**, a nie jeszcze uniwersalną platformą pluginów. Jego realna wartość to: jeden proces hosta, wspólny sposób obsługi komend, presety kontekstu pracy i odporność UI na śmierć pluginu. Najbardziej sensowny kierunek to prywatny/power-userowy kokpit dla lokalnych workflowów: Rust + system + audio + lekki recon. Nie warto próbować konkurować z dojrzałymi narzędziami w pojedynczych domenach.

## Co faktycznie jest zaimplementowane

- `host_engine`: Ratatui/Crossterm; wybór presetu, przełączanie slotów, pole komend, tick co 250 ms, reload `workspace.toml` i powrót do presetów.
- Każdy plugin jest osobnym procesem potomnym. Host czyta stdout w osobnym wątku; EOF z niedomkniętą ramką daje status `CRASH`. `CRUCIBLE` celowo wysyła urwaną ramkę i kończy się po 6 tickach — to realny test ścieżki awarii.
- `shared_contract`: tekstowe ramki JFP v5.4.0 po stdio, limit 1 MiB na ramkę i 7 testów parsera. `cargo test --workspace` przechodzi: **7/7 testów**, lecz nie ma testów hosta ani pluginów.
- Presety działają jako prosta selekcja pluginów: DevOps, Audio, Recon, Wszystko. To nie jest jeszcze automatyzacja ani pipeline między pluginami.
- `forge.sh` kopiuje szablon, dodaje member do Cargo workspace, zależności, buduje i dopisuje slot — dobry bootstrap, lecz nie rejestruje pluginu w presetach i jest zależny od układu tekstu manifestu.

### Aktualne możliwości

| Moduł | Co robi naprawdę | Granica |
|---|---|---|
| OSINT | Pyta `ip-api.com` o IP/domenę i pokazuje JSON | To geolokalizacja/reputacja z jednego API, nie OSINT w pełnym sensie; używa HTTP, nie HTTPS. |
| SEARCH | Pobiera URL i wyciąga title, description, H1–H3, do 30 linków | Nie jest wyszukiwarką WWW; nie renderuje JS, nie ma crawl/paginacji. |
| AUDIO | Czyta WAV, długość, peak/RMS/crest | Skanuje cały plik synchronicznie; tylko WAV. |
| SCOUT | Szuka crates.io, sortuje, pokazuje detale | Brak oceny jakości/bezpieczeństwa zależności. |
| SYSINFO | CPU/RAM/swap/dyski/sieć/procesy na komendę | Snapshoty bez historii, alertów i wykresów. |
| MEDIA | Czyta tagi i właściwości MP3/FLAC/WAV/OGG | Tylko odczyt; brak edycji/batch. |

## Jakie problemy rozwiązuje

1. **Przełączanie kontekstu bez otwierania pięciu terminali.** Preset Audio daje analizę WAV i tagów, DevOps — system, sieć i crates.io. To oszczędza orientację, nie obliczenia.
2. **Jednolity interfejs dla własnych mikro-narzędzi.** Autor pluginu dostaje stdin/stdout, `COMMAND`/`TICK`, szablon oraz rejestr slotu, zamiast za każdym razem budować TUI od zera.
3. **Awaria narzędzia nie zabija workbencha.** Każdy plugin jest procesem; host wykrywa problem ramki/EOF i można przełączyć moduł. To jest praktyczne przy eksperymentalnym kodzie i parserach mediów.
4. **Szybki rekonesans danych lokalnych i sieciowych.** Przykład: sprawdź procesy/obciążenie, odczytaj metadane pliku, sprawdź IP/domenę, wyszukaj crate — bez zmiany mentalnego modelu interfejsu.

Nie rozwiązuje: zarządzania projektami, automatycznej orkiestracji, współdzielenia wyników, długich zadań, bezpiecznego skanowania ani pełnego monitoringu.

## Kto realnie skorzysta

- **Solo Rust hacker / twórca agentów lokalnych:** podczas pracy nad narzędziami na desktopie; szybko dobiera crate, patrzy na procesy i testuje adapter.
- **Audio developer / archiwista:** porównuje pliki WAV i tagi w folderach, potem potrzebuje batch/reportów.
- **DevOps/SRE w małej skali:** doraźny lokalny cockpit na maszynie deweloperskiej, nie substytut Grafany/Prometheusa.
- **Osoba ucząca się Rust:** świetny materiał na granice procesu, IPC, TUI, kontrakty i crates — plugin jest mały i czytelny.
- **Nie jest dla:** zespołu chcącego centralnych danych, pentestera potrzebującego specjalistycznego reconu, ani użytkownika, który oczekuje gotowego zamiennika `bottom`, `lazygit` czy DAW.

Pulpit Jaro wskazuje, że dopasowanie jest realne: są projekty audio/audiobooków, OSINT, RUST LAB, AGENTIC OS, JFP BOX, monitoring/defence i liczne eksperymenty z agentami. Najlepszy produkt to więc osobisty „launcher narzędzi roboczych”, nie ogólny marketplace.

## Nowe pluginy — kolejność wg impactu

1. **PLUGIN_GIT_STATUS** — `git2`, `gix`: status, diff stat, branch, ostatnie commity; największa codzienna użyteczność.
2. **PLUGIN_PROJECT_DOCTOR** — `walkdir`, `ignore`, `toml`, `serde_yaml`: wykrywa Cargo/npm/Python, komendy test/build, braki plików.
3. **PLUGIN_LOG_TAIL** — `notify`, `globset`, `regex`: tail wielu logów, filtr, rate/error count; pasuje do modeli agentowych.
4. **PLUGIN_DEP_AUDIT** — `cargo_metadata`, `rustsec`, `guppy`: zależności, advisory, licencje, duplikaty.
5. **PLUGIN_HTTP_LAB** — `reqwest`, `serde_json`, `jsonpath-rust`: requesty, nagłówki, JSON, zapisane presety API.
6. **PLUGIN_FILE_HUNTER** — `ripgrep` (biblioteka), `ignore`, `regex`: szybkie wyszukiwanie kodu/notatek z wynikami.
7. **PLUGIN_AUDIO_BATCH** — `symphonia`, `rubato`, `walkdir`: batchowy katalog audio, normalizacja metryk i CSV/JSON.
8. **PLUGIN_MEDIA_QC** — `ffprobe-rs` lub wywołanie `ffprobe`, `serde_json`: kodek, głośność, brak tagów, anomalie.
9. **PLUGIN_NET_DIAG** — `trust-dns-resolver`, `pnet`, `socket2`: DNS, port reachability, interfejsy; bez udawania pełnego skanera.
10. **PLUGIN_WORKSPACE_INDEX** — `tantivy`, `walkdir`, `ignore`: lokalny indeks dokumentacji/notatek, wyniki z fragmentami.
11. **PLUGIN_SQLITE_INSPECT** — `rusqlite`, `csv`: przegląd schematu, zapytania readonly, eksport.
12. **PLUGIN_DIFF_REPORT** — `similar`, `walkdir`: różnice drzew/projektów i raport zmian.
13. **PLUGIN_SECRET_GUARD** — `gitleaks` jako subprocess lub `regex`, `entropy`: skan przed commitem; domyślnie tylko raport.
14. **PLUGIN_JFP_TRACE** — `tracing`, `tracing-subscriber`: rejestr ramek, czasów i crashy pluginów — konieczny do dojrzewania platformy.
15. **PLUGIN_TASK_RUNNER** — `tokio`, `serde`, `shell-words`: deklaratywne, zatwierdzane taski projektu; dopiero po dodaniu kontroli uprawnień.

## Porównanie z istniejącymi TUI

| Narzędzie | Jest świetne w | XcratesX wobec niego |
|---|---|---|
| [lazygit](https://lazygit.dev/docs/) | pełnym workflow Git: staging, rebase, commit, UI pod Git | Nie konkuruje. Powinien pluginem pokazać skrót/status lub uruchamiać lazygit. |
| [bottom](https://github.com/ClementTsang/bottom) | wykresach procesów, CPU/RAM/dysku/sieci i dojrzałym monitorze | `SYSINFO` jest snapshotem i debug-helperem; bottom jest lepszy do obserwacji na żywo. |
| [dust](https://github.com/bootandy/dust) | szybkim, czytelnym użyciu dysku | Brak odpowiednika; nie należy pisać duplikatu, lepiej dodać adapter/wywołanie. |
| `curl`/`jq`/`ripgrep` | pojedynczym zadaniu i skryptowaniu | XcratesX wygrywa tylko spójnym menu, presetem i przyjaznym wynikiem. |

**Killer feature (jeśli zostanie dowieziony):** nie „TUI z pluginami”, tylko *presetowy, izolowany kokpit prywatnych narzędzi* — z jednym kontraktem, raportami i możliwością dopisywania domenowych adapterów w godzinę. Obecnie jest to obietnica kierunku, nie jeszcze przewaga nad osobnymi programami.

## Potencjał

- **Open source: wysoki jako framework/learning project**, jeśli protokół i SDK zostaną stabilne, a przykłady będą konkretne. Zyskuje przez małe, audytowalne pluginy.
- **Komercyjny: niski jako samo TUI.** Płatne mogłyby być firmowe pakiety pluginów, lokalne polityki, dashboardy i wsparcie — ale wymaga to auth, aktualizacji i bezpieczeństwa.
- **Edukacyjny: bardzo wysoki.** Jeden repo pokazuje workspace Cargo, IPC, parsowanie strumienia, async I/O, procesy potomne i TUI bez abstrakcji zasłaniającej mechanikę.

## Słabości i priorytety naprawy

1. **Brak wersjonowanej specyfikacji i capability modelu.** JFP ma stałą `VERSION`, lecz parser jej nie waliduje; nie ma handshake, manifestu możliwości, schematu wejścia/wyjścia ani semver pluginu.
2. **Bezpieczeństwo sieci:** OSINT wysyła cel przez zwykły HTTP do zewnętrznego API; SEARCH pobiera dowolny URL bez allowlisty, limitu rozmiaru, timeoutu czy ochrony przed dostępem do localhost/sieci prywatnej (SSRF, nawet lokalnie).
3. **Brak procesu instalacji/discovery.** Plugin jest wpisem w Cargo, binarką w `target/debug` i slotem w TOML. Nie ma manifestu pluginu, release path, kompatybilności ani katalogu użytkownika.
4. **Brak observability.** stderr jest wyrzucony do `null`, więc błąd/panic pluginu traci diagnozę; host widzi głównie EOF. Zapisuj stderr, exit code, czas i ostatnie ramki.
5. **Pętla 4 Hz i pełne odpowiedzi.** Każdy tick może generować duży tekst; CRUCIBLE demonstruje 500 kB ramek. Host zachowuje tylko ostatnią odpowiedź, więc nie ma backpressure, streamingu ani historii.
6. **`forge.sh` jest szybki, ale kruchy.** Edytuje TOML przez `sed`, numeruje sloty przez liczbę bloków (po usunięciu slotów możliwy konflikt), nie aktualizuje presetów i zakłada `cargo add`.
7. **Jakość modułów jest asymetryczna.** AUDIO blokuje odczyt całego WAV w głównej pętli; SYSINFO odświeża pełny stan; SEARCH nie radzi sobie z JS; MEDIA tylko czyta. To dobre proof-of-concepty, nie narzędzia produkcyjne.
8. **Brak testów end-to-end i kontraktowych pluginów.** Jest 7 testów parsera, zero testów scenariuszy: spawn → COMMAND → TICK → crash → restart, timeouty, złe wiadomości, duże odpowiedzi.
9. **Brak trwałości i eksportu.** Wyniki nie trafiają do pliku/JSON/Markdown; presety nie mają parametrów ani zmiennych środowiskowych.

### Plan na najbliższe 3 iteracje

1. Dodać `plugin.toml`, handshake JFP, log stderr/exit code, timeouty i test E2E z Crucible.
2. Utwardzić HTTP (HTTPS, timeout, limit body, blokada prywatnych adresów domyślnie) oraz dołożyć eksport raportów.
3. Zbudować trzy pluginy codzienne: Git status, Project Doctor, Log Tail. Dopiero potem myśleć o publicznych pluginach lub marketplace.

## Źródła i weryfikacja

- Kod lokalny: `host_engine/src/main.rs`, `shared_contract/src/lib.rs`, `workspace.toml`, `presets.toml`, `forge.sh` oraz implementacje pluginów.
- [Dokumentacja lazygit](https://lazygit.dev/docs/) — zakres pełnego GUI Git.
- [bottom — oficjalne repozytorium](https://github.com/ClementTsang/bottom) — punkt odniesienia dla TUI monitora systemu.
- [dust — oficjalne repozytorium](https://github.com/bootandy/dust) — punkt odniesienia dla analizy użycia dysku.
