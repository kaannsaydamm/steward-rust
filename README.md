# Steward

Local-first agent operations runtime with a Rust daemon, CLI/TUI, governed tools, portable state, and an optional web console.

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

### Quick start from source

Requirements: stable Rust, Node.js 20+ for the optional web console, and PowerShell 7 on Windows.

```powershell
cargo build --release -p steward-cli -p steward-daemon
./target/release/steward-cli.exe
```

The CLI starts the local daemon automatically. Useful commands:

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

```powershell
cd web-ui
npm install
npm run build
npm run start
```

Open `http://127.0.0.1:3000`. The daemon only binds to loopback and accepts browser origins from loopback hosts.

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

---

## Türkçe

Steward; Rust daemon, CLI/TUI, yönetilen araçlar ve isteğe bağlı web konsolu içeren local-first bir agent operasyon runtime'ıdır. Tüm değişken veriler varsayılan olarak `~/.steward` altında tutulur.

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
cargo build --release -p steward-cli -p steward-daemon
./target/release/steward-cli.exe
```

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

---

## Русский

Steward — локальная среда управления агентами с Rust-демоном, CLI/TUI, контролируемыми инструментами и дополнительной веб-консолью. Все изменяемые данные хранятся в `~/.steward`.

### Возможности

- устойчивые workflow с подтверждением, отменой, журналами и восстановлением;
- локальная память и поиск;
- политика разрешений и ограниченный аудит вызовов инструментов;
- самоподписанные пакеты навыков без списка доверенных издателей;
- регистрация, обнаружение и запуск stdio MCP-адаптеров;
- перенос всех сессий и настроек через import/export.

### Запуск

```powershell
cargo build --release -p steward-cli -p steward-daemon
./target/release/steward-cli.exe
```

```powershell
./scripts/package.ps1
Expand-Archive ./dist/steward-windows-x64.zip ./dist/steward
./dist/steward/install.ps1
```

Повторный запуск установщика обновляет программу и сохраняет `~/.steward`. Архитектура: [ARCHITECTURE.md](ARCHITECTURE.md).

---

## Français

Steward est un environnement local d'exploitation d'agents comprenant un démon Rust, une CLI/TUI, des outils gouvernés et une console web facultative. Toutes les données modifiables restent dans `~/.steward`.

### Fonctions principales

- workflows persistants avec approbation, annulation, journaux et reprise;
- mémoire locale interrogeable;
- exécution d'outils avec politique d'approbation et audit borné;
- bundles de compétences auto-signés sans liste d'éditeurs autorisés;
- adaptateurs MCP stdio avec découverte et cycle start/stop;
- import/export complet des sessions et de la configuration.

### Démarrage

```powershell
cargo build --release -p steward-cli -p steward-daemon
./target/release/steward-cli.exe
```

```powershell
./scripts/package.ps1
Expand-Archive ./dist/steward-windows-x64.zip ./dist/steward
./dist/steward/install.ps1
```

Relancer l'installateur met à jour l'application sans supprimer `~/.steward`. Architecture: [ARCHITECTURE.md](ARCHITECTURE.md).

---

## Deutsch

Steward ist eine lokal betriebene Agenten-Laufzeit mit Rust-Daemon, CLI/TUI, kontrollierten Werkzeugen und optionaler Web-Konsole. Veränderliche Daten liegen vollständig unter `~/.steward`.

### Funktionen

- dauerhafte Workflows mit Freigabe, Abbruch, Logs und Wiederaufnahme;
- lokal durchsuchbarer Speicher;
- richtlinienbasierte Tool-Ausführung mit begrenztem Audit-Verlauf;
- selbstsignierte Skill-Pakete ohne Herausgeber-Allowlist;
- stdio-MCP-Adapter mit Discovery und Start/Stop-Lebenszyklus;
- vollständiger Import/Export von Sitzungen und Konfiguration.

### Start

```powershell
cargo build --release -p steward-cli -p steward-daemon
./target/release/steward-cli.exe
```

```powershell
./scripts/package.ps1
Expand-Archive ./dist/steward-windows-x64.zip ./dist/steward
./dist/steward/install.ps1
```

Erneutes Ausführen des Installers aktualisiert die Anwendung und erhält `~/.steward`. Architektur: [ARCHITECTURE.md](ARCHITECTURE.md).

---

## Español

Steward es un entorno local de operación de agentes con daemon en Rust, CLI/TUI, herramientas gobernadas y una consola web opcional. Todos los datos mutables permanecen en `~/.steward`.

### Funciones

- workflows duraderos con aprobación, cancelación, logs y recuperación;
- memoria local consultable;
- ejecución de herramientas con políticas y auditoría limitada;
- paquetes de skills autofirmados sin lista de editores permitidos;
- adaptadores MCP stdio con descubrimiento y ciclo start/stop;
- importación y exportación completa de sesiones y configuración.

### Inicio

```powershell
cargo build --release -p steward-cli -p steward-daemon
./target/release/steward-cli.exe
```

```powershell
./scripts/package.ps1
Expand-Archive ./dist/steward-windows-x64.zip ./dist/steward
./dist/steward/install.ps1
```

Ejecutar de nuevo el instalador actualiza la aplicación y conserva `~/.steward`. Arquitectura: [ARCHITECTURE.md](ARCHITECTURE.md).
