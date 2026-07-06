# Steward — proje notları

## Vectordb / graph db (claude-flow)

Bu proje için `mcp__claude-flow__*` hafıza katmanı `namespace="steward"` altında tutuluyor (global `~/.claude/CLAUDE.md` § 0.1'deki kurala göre). Fiziksel olarak proje-lokal bir `.swarm/`/`.claude-flow/` yok (bkz. o bölümdeki not) — izolasyon namespace ile sağlanıyor.

- Session başında: `memory_search`/`memory_list` ile `namespace=steward` sorgula, önce oradan oku.
- Görev başı/sonu: `hooks_pre-task` / `hooks_post-task`.
- Kalıcı bilgi (mimari karar, çözülen bug, tamamlanan feature): `memory_store` (`namespace=steward`, `upsert:true`).

Dosya-tabanlı oturum hafızası (`~/.claude/projects/.../memory/`) ayrıca ve paralel olarak kullanılıyor — vectordb bunun yerine değil, yanında.

## Mimari

`ARCHITECTURE.md` dosyasına bakınız — Rust workspace (`crates/core`, `crates/daemon`, `crates/cli`, `crates/knowledge`) + Next.js static-export WebUI (`web-ui/`), tümü tek bir daemon (gRPC + grpc-web + axum web server) ile konuşuyor.
