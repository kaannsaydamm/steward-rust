# Steward

Local-first agent runtime with a Rust daemon, streaming CLI/TUI, governed tools, portable state, and a responsive web console.

[![CI](https://github.com/kaannsaydamm/steward/actions/workflows/ci.yml/badge.svg)](https://github.com/kaannsaydamm/steward/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

[English](#english) · [Türkçe](#türkçe) · [Русский](#русский) · [Français](#français) · [Deutsch](#deutsch) · [Español](#español)

---

## English

### What it does

Steward runs locally and keeps its mutable state under `~/.steward`. It provides:

- durable workflows with approval, cancellation, logs, and restart recovery;
- short/long/reasoning/negative/dream memory with local search;
- policy-enforced tool execution with approval and bounded audit history;
- self-signed skill bundles without a publisher allowlist;
- stdio MCP adapter registration, discovery, start/stop, and audited invocation;
- the interactive Butler terminal UI and a responsive web operator console;
- complete `.steward` import/export for moving sessions and configuration.
- OpenAI-compatible, Anthropic Messages, and Google Gemini model adapters with portable provider profiles;
- persisted model sessions with resume/delete support and streamed model/tool events;
- validated DAG workflows editable by drag-and-drop or JSON and executed node-by-node by the model runtime.

The provider catalog contains direct API templates and editable compatibility endpoints. OAuth subscription reuse and vendor CLI bridges are not presented as native support; use a compatible local endpoint when a vendor does not expose a direct API.

### Project status

Steward is under active development and has not reached 1.0. Commands, configuration fields, APIs, and persisted state formats may change between releases. Back up `~/.steward` before upgrades or migration testing.

### Quick start from source

Requirements: stable Rust, Node.js 20+ for building the web console, and PowerShell 7 on Windows.

```powershell
cd web-ui; npm ci; npm run build; cd ..
cargo build --release -p steward-cli -p steward-daemon
./target/release/steward-cli.exe
```

On first launch, Steward opens a guided setup. Afterwards, `steward` starts the daemon and embedded Web UI automatically, then opens the terminal interface. Re-run setup at any time with `steward setup`; use `steward setup --quick` for defaults in unattended installs.

npm launcher, when the package is available on npm:

```powershell
npx -y @kaannsaydamm/steward
```

The launcher caches signed release contents under `~/.steward/runtime`. Maintainers publish the launcher with `npm publish --access public` from this repository.

Useful commands:

```powershell
steward ping
steward status
steward doctor --strict
steward workflow start "Review this repository"
steward tools list
steward mcp list
steward maintenance status
```

### Portable Windows package

```powershell
./scripts/package.ps1
Expand-Archive ./dist/steward-windows-x64.zip ./dist/steward
./dist/steward/install.ps1
steward
```

Re-run `install.ps1` from a newer archive to update. Uninstall preserves `~/.steward` unless `-RemoveData` is explicitly supplied:

```powershell
& "$env:LOCALAPPDATA/Steward/uninstall.ps1"
```

### Web console

The daemon serves the packaged Web UI at `http://127.0.0.1:3000`; the terminal prints the configured address on launch. Both services bind to loopback only. A custom local Web UI port can be selected with `steward setup`.

### Data, retention, and portability

Default configuration (`~/.steward/config.json`):

```json
{"retention_days":30,"max_completed_workflows":200}
```

```powershell
steward maintenance prune
steward data export backup.steward.zip
steward data import backup.steward.zip
```

Active and approval-waiting workflows are not pruned. MCP processes never auto-start after a daemon restart.

### Development verification

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cd web-ui
npm run lint
npm run build
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for runtime boundaries and persistence design.

### Contributing, security, and license

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting a substantial change and follow the [Code of Conduct](CODE_OF_CONDUCT.md) in project spaces.

Report suspected vulnerabilities privately according to [SECURITY.md](SECURITY.md); do not publish vulnerability details in an issue.

Steward is open-source software licensed under the [MIT License](LICENSE). Copyright © 2026 Kaan Kadir Aluçlu.

---

## Türkçe

Steward; Rust daemon, streaming CLI/TUI, yönetilen araçlar ve responsive web konsolu içeren local-first bir agent runtime'dır. Tüm değişken veriler varsayılan olarak `~/.steward` altında tutulur. OpenAI-compatible, Anthropic ve Gemini adapter'ları; kalıcı session'lar ve görsel/JSON DAG workflow çalıştırma desteği içerir.

### Özellikler

- Onay, iptal, log ve yeniden başlatma kurtarmalı kalıcı workflow'lar
- Yerel aranabilir bellek ve nightly dream çıktıları
- Onay politikası ve sınırlı audit geçmişi olan araç çalıştırma
- Publisher allowlist gerektirmeyen self-signed skill paketleri
- stdio MCP kayıt, keşif, başlatma/durdurma ve auditli invocation
- Responsive web konsolu ve Butler interaktif terminal arayüzü
- Session/config dahil tüm `.steward` verisini import/export

### Çalıştırma

```powershell
cd web-ui; npm ci; npm run build; cd ..
cargo build --release -p steward-cli -p steward-daemon
./target/release/steward-cli.exe
```

İlk çalıştırmada kurulum sihirbazı açılır. Sonraki çalıştırmalarda `steward`, daemon ile Web UI'ı otomatik başlatıp terminal arayüzünü açar; ayarlar `steward setup` ile yeniden düzenlenir. Web adresi varsayılan olarak `http://127.0.0.1:3000`'dir.

Windows paketi:

```powershell
./scripts/package.ps1
Expand-Archive ./dist/steward-windows-x64.zip ./dist/steward
./dist/steward/install.ps1
steward
```

Güncellemek için yeni paketteki `install.ps1` tekrar çalıştırılır. Normal kaldırma `~/.steward` verisini korur.

```powershell
steward maintenance status
steward data export backup.steward.zip
```

Mimari ayrıntılar: [ARCHITECTURE.md](ARCHITECTURE.md).

Katkı kuralları için [CONTRIBUTING.md](CONTRIBUTING.md), güvenlik bildirimleri için [SECURITY.md](SECURITY.md) ve lisans koşulları için [LICENSE](LICENSE) dosyalarına bakın.

---

## Русский

Steward — локальная агентная среда с Rust-демоном, потоковым CLI/TUI, контролируемыми инструментами и адаптивной веб-консолью. Все изменяемые данные хранятся в `~/.steward`. Поддерживаются OpenAI-совместимые API, Anthropic Messages, Gemini, постоянные сессии и исполняемые DAG-workflow в визуальном и JSON-редакторах.

### Возможности

- устойчивые workflow с подтверждением, отменой, журналами и восстановлением;
- локальная память и поиск;
- политика разрешений и ограниченный аудит вызовов инструментов;
- самоподписанные пакеты навыков без списка доверенных издателей;
- регистрация, обнаружение и запуск stdio MCP-адаптеров;
- перенос всех сессий и настроек через import/export.

### Запуск

```powershell
cd web-ui; npm ci; npm run build; cd ..
cargo build --release -p steward-cli -p steward-daemon
./target/release/steward-cli.exe
```

При первом запуске открывается мастер настройки. Затем `steward` автоматически запускает демон и Web UI, выводит адрес `http://127.0.0.1:3000` и открывает терминал; повторная настройка доступна через `steward setup`.

```powershell
./scripts/package.ps1
Expand-Archive ./dist/steward-windows-x64.zip ./dist/steward
./dist/steward/install.ps1
```

Повторный запуск установщика обновляет программу и сохраняет `~/.steward`. Архитектура: [ARCHITECTURE.md](ARCHITECTURE.md).

---

## Français

Steward est un runtime d'agents local comprenant un démon Rust, une CLI/TUI en streaming, des outils gouvernés et une console web responsive. Toutes les données modifiables restent dans `~/.steward`. Il prend en charge les API compatibles OpenAI, Anthropic Messages, Gemini, les sessions persistantes et les workflows DAG exécutables en mode visuel ou JSON.

### Fonctions principales

- workflows persistants avec approbation, annulation, journaux et reprise;
- mémoire locale interrogeable;
- exécution d'outils avec politique d'approbation et audit borné;
- bundles de compétences auto-signés sans liste d'éditeurs autorisés;
- adaptateurs MCP stdio avec découverte et cycle start/stop;
- import/export complet des sessions et de la configuration.

### Démarrage

```powershell
cd web-ui; npm ci; npm run build; cd ..
cargo build --release -p steward-cli -p steward-daemon
./target/release/steward-cli.exe
```

Au premier lancement, un assistant de configuration s'ouvre. Ensuite, `steward` démarre automatiquement le démon et l'interface Web, affiche `http://127.0.0.1:3000` et ouvre le terminal; `steward setup` relance la configuration.

```powershell
./scripts/package.ps1
Expand-Archive ./dist/steward-windows-x64.zip ./dist/steward
./dist/steward/install.ps1
```

Relancer l'installateur met à jour l'application sans supprimer `~/.steward`. Architecture: [ARCHITECTURE.md](ARCHITECTURE.md).

---

## Deutsch

Steward ist eine lokale Agent-Runtime mit Rust-Daemon, Streaming-CLI/TUI, kontrollierten Werkzeugen und responsiver Web-Konsole. Veränderliche Daten liegen vollständig unter `~/.steward`. Unterstützt werden OpenAI-kompatible APIs, Anthropic Messages, Gemini, persistente Sitzungen und ausführbare DAG-Workflows im visuellen oder JSON-Editor.

### Funktionen

- dauerhafte Workflows mit Freigabe, Abbruch, Logs und Wiederaufnahme;
- lokal durchsuchbarer Speicher;
- richtlinienbasierte Tool-Ausführung mit begrenztem Audit-Verlauf;
- selbstsignierte Skill-Pakete ohne Herausgeber-Allowlist;
- stdio-MCP-Adapter mit Discovery und Start/Stop-Lebenszyklus;
- vollständiger Import/Export von Sitzungen und Konfiguration.

### Start

```powershell
cd web-ui; npm ci; npm run build; cd ..
cargo build --release -p steward-cli -p steward-daemon
./target/release/steward-cli.exe
```

Beim ersten Start erscheint der Einrichtungsassistent. Danach startet `steward` Daemon und Web UI automatisch, zeigt `http://127.0.0.1:3000` an und öffnet das Terminal; `steward setup` öffnet die Einrichtung erneut.

```powershell
./scripts/package.ps1
Expand-Archive ./dist/steward-windows-x64.zip ./dist/steward
./dist/steward/install.ps1
```

Erneutes Ausführen des Installers aktualisiert die Anwendung und erhält `~/.steward`. Architektur: [ARCHITECTURE.md](ARCHITECTURE.md).

---

## Español

Steward es un runtime de agentes local con daemon en Rust, CLI/TUI con streaming, herramientas gobernadas y una consola web responsive. Todos los datos mutables permanecen en `~/.steward`. Incluye APIs compatibles con OpenAI, Anthropic Messages, Gemini, sesiones persistentes y workflows DAG ejecutables mediante editor visual o JSON.

### Funciones

- workflows duraderos con aprobación, cancelación, logs y recuperación;
- memoria local consultable;
- ejecución de herramientas con políticas y auditoría limitada;
- paquetes de skills autofirmados sin lista de editores permitidos;
- adaptadores MCP stdio con descubrimiento y ciclo start/stop;
- importación y exportación completa de sesiones y configuración.

### Inicio

```powershell
cd web-ui; npm ci; npm run build; cd ..
cargo build --release -p steward-cli -p steward-daemon
./target/release/steward-cli.exe
```

En el primer inicio se abre el asistente de configuración. Después, `steward` inicia automáticamente el daemon y la Web UI, muestra `http://127.0.0.1:3000` y abre el terminal; `steward setup` vuelve a abrir la configuración.

```powershell
./scripts/package.ps1
Expand-Archive ./dist/steward-windows-x64.zip ./dist/steward
./dist/steward/install.ps1
```

Ejecutar de nuevo el instalador actualiza la aplicación y conserva `~/.steward`. Arquitectura: [ARCHITECTURE.md](ARCHITECTURE.md).
