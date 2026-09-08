# AGENTS.md — Steward Rust

Agent'lar (Claude Code, Codex, Gemini ve benzeri) bu repoda çalışırken uyacağı
kurallar. İnsan okuyucu için mimari özet: `ARCHITECTURE.md`; Omega planı
uygulama kayıtları: `docs/PARITY.md`, `docs/SECURITY_ARCHITECTURE.md`.

## Komutlar

```bash
cargo build --workspace                 # derle
cargo test --workspace -- --test-threads=1   # tüm testler (485+); seri çalışır, bazı store testleri portu paylaşır
cargo test -p steward-kernel            # tek crate
RUSTFLAGS="-A clippy::result_large_err" cargo clippy --workspace --all-targets   # lint (tonic istisnası)
cargo fmt --all                         # format
cd web-ui && npm run build              # WebUI (Next.js static export → web-ui/out)
```

Canlı provider evals (gerçek model çağrısı, maliyetli — sadece istenirse):

```bash
STEWARD_TEST_BASE_URL=... STEWARD_TEST_API_KEY=... cargo test -p steward-e2e-tests --test v1_live
```

## Mimari (kısa)

Rust workspace, tek daemon (`crates/daemon` = gRPC + gRPC-Web + axum statik
WebUI), çok istemci: `crates/cli` (TUI), `web-ui/` (Next.js), `desktop/`
(Tauri supervisor).

| Crate | Sorumluluk |
|---|---|
| `core` | storage, provider config, **SecretStore (OS vault)**, secret redaction |
| `daemon` | RPC katmanı, stores (threads/runs/events/checkpoints), `STEWARD_AGENT_RUNTIME=omega` flag'i ile kernel routing, db migrator (versiyonlu + backup), import/export |
| `kernel` | TurnEngine, Execution IR + scheduler, HITL, signals, trace, RLM sidecar |
| `context` | token bütçeli context allocator, manifest, path-scoped rules, compaction |
| `models` | capability catalog, router, fallback ladder |
| `tools` | effect-based policy, scoped approvals, lazy discovery, parallel batch |
| `workspace` | lease'ler, ProcessManager, git worktree manager |
| `coding` | repo index, unified reader, hash-anchored edits, symbol graph, LSP/DAP |
| `harness` | AgentProfile, subagent scheduler, mailbox, Git-backed context repo, memory, skills, dream/refine, hooks |
| `wire` | Wire v2 envelope + handshake (`/api/v2/wire` WebSocket) |
| `evals` | deterministic ScriptedModel + parity suite |
| `desktop/src-tauri` | Tauri supervisor: daemon sağlık kontrolü → attach/spawn |

## Zorunlu kurallar

1. **Secrets asla plain JSON'a yazılmaz.** Provider API key'leri OS vault'a
   girer (`crates/core/src/secrets.rs`); `providers.json`'da yalnız
   `vault:` referansı durur. Test fixture'larında gerçek key bulunmaz.
2. **Tool policy effect-tabanlıdır**, isim-tabanlı değil. Yeni tool eklerken
   `ToolSpec.effects` doldurulur; alias'lar bypass sayılmaz.
3. **Hash-anchored edit** sözleşmesi: revision + satır ankoru doğrulanır
   (`crates/coding/src/edit.rs`); düzenleme araçları bu sözleşmeyi bozmaz.
4. **DB şema değişikliği** → yeni versiyonlu migration + backup adımı
   (`crates/daemon/src/db_migrator.rs`). Doğrudan `CREATE TABLE` yok.
5. **Wire mesajları** `crates/wire` envelope'undan geçer; v1 uyumluluk
   `app_server/compat_v1.rs`'te izole.
6. **Kernel hot loop'u** sabit tur limitiyle sınırlandırılmaz; bütçe
   sonlandırır. `for i in 0..N { model.call() }` kalıbı geri getirilmez.
7. **Windows birinci sınıf platformdur.** Yol ayrımı, proses yönetimi ve
   tempfile ömürleri Windows'ta test edilir.
8. **Her davranış değişikliği testle gelir.** Davranışsal sözleşme
   (invariant, boundary, hata yolu) test edilir; kaynak metni değil.

## Shared store (`~/.steward`)

Tek gerçek kaynak: Rust daemon'ın `~/.steward` dizini. Tüm yüzeyler aynı
store'u okur/yazar:

| Dosya/Dizin | Sahip | Okuyanlar |
|---|---|---|
| `steward.db` | daemon | daemon (thread/run/event store) |
| `wire-port`, `wire-token` | daemon | CLI, gateway bridge (daemon keşfi) |
| `providers.json` | daemon | daemon (SecretStore `vault:` referansları) |
| `agents/*.yaml` | daemon (gRPC Save/List/DeleteAgentProfile) | CLI, desktop `/builder`, web `/builder` |

Yüzeyler ve store yolları:

- **CLI** (`steward` npm launcher → vendor/omp): daemon gRPC 50051; oturumlar
  `steward.db`'de.
- **Desktop** (vendor/hermes/apps/desktop Electron): `hermes serve` backend'ini
  `HERMES_HOME=~/.steward` ile spawn eder (`resolveHermesHome()` default'u —
  `electron/main.ts`). Agent Builder `/builder` → gateway JSON-RPC
  `steward.profiles.*` (`gateway/steward_bridge/run_agent.py`) → daemon gRPC.
- **Web dashboard** (`hermes dashboard` :8093): SPA `/builder` → REST
  `/api/steward/profiles` (`hermes_cli/web_routers/steward_bridge.py`) → daemon
  gRPC (stub'lar `gateway/steward_bridge/stubs`, `STEWARD_DAEMON_ADDR`
  default `127.0.0.1:50051`).

HERMES_HOME env değişkeni her yerde override olarak kazanır; unset ise tüm
yüzeyler `~/.steward`'a düşer (desktop + `steward web` launcher bunu garanti
eder). Böylece oturumlar (daemon store) ve AgentProfile'lar (daemon store)
yüzeyler arası tek kalır.

## Test düzeni

- `crates/*/src/*_tests.rs`: birim testler, `#[path]` include ile crate'e bağlı
- `tests/`: e2e + parity (`v1_live` canlı, env-gated)
- Test adları davranışı anlatır: `oversized_module_is_rejected_before_compilation`
  biçimi (snake_case cümle).

## Commit protokolü

- Conventional commits: `feat(scope):`, `fix(scope):`, `docs(scope):`
- Bir mantıksal değişiklik = bir commit; testler commit'te yeşil olmalı.

## Yapma listesi

- Plan dosyalarını repoya kopyalama.
- `Cargo.lock`'u elle düzenleme.
- `web-ui/out/` build çıktısını commit'leme.
- Tonik üretim dosyalarını elle düzenleme.
- Hook'ları policy bypass'ı olarak kullanma.
