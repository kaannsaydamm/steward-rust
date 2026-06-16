# Steward Agent OS — v2 Architecture

> Tarih: 2026-06-16
> Bu doküman, agent framework araştırması, graph DB trendleri ve kullanıcının workflow talebi doğrultusunda
> Steward'ın yeni mimarisini tanımlar.

---

## 1. Mevcut Durum Analizi

Steward şu anda:
- **Rust daemon** (tonic gRPC) → SQLite + Wasmtime sandbox
- **Rust CLI** (ratatui TUI) → daemon'a gRPC
- **Web UI** (Next.js 16) → gRPC-web ile daemon'a bağlanıyor
- **3 RPC**: Ping, ExecuteTask, RunPlugin

### Eksikler / Yapılacaklar
- [ ] Workflow engine (gelişmiş agent orchestrasyonu) yok
- [ ] Graph DB / knowledge graph yok (sadece düz SQLite)
- [ ] Agent memory sistemi yok
- [ ] Web UI çok basit (sadece ping + task submit)
- [ ] CLI/TUI interaktif değil (sadece welcome + exit)

---

## 2. Agent Framework Research — Özet

| Framework | Tür | Çoklu-Ajan | Kalıcılık | Graph DB | Workflow | Dil |
|-----------|-----|-----------|-----------|---------|----------|-----|
| **LangGraph** | Execution framework | ✅ (graph nodes) | PostgreSQL/Redis | Add-on | Stateful graph | Python/TS |
| **CrewAI** | Multi-agent orchestration | ✅ (Crews) | ❌ Yok | Add-on | Task-based | Python only |
| **OpenAI Agents SDK** | Minimal agent SDK | ✅ (handoffs) | ❌ Yok | ❌ Yok | Handoff zinciri | Python/TS |
| **AutoGen → MS Agent Framework** | Multi-agent | ✅ (GroupChat) | ❌ Yok | ❌ Yok | Conversational | Python/.NET |
| **Mastra** | TS agent framework | Limited | ❌ | ❌ | Workflow | TS only |

### Tespit Edilen Boşluklar (Pazar Boşluğu)

1. **Hiçbir framework Rust-native değil** — hepsi Python/JS
2. **Kalıcılık çoğunda yok** — CrewAI, AutoGen, OpenAI SDK'da built-in persistence yok
3. **Graph DB entegrasyonu** — hep add-on/third-party, native değil
4. **Workflow + Agent + Memory** üçlüsü tek üründe yok
5. **Local-first / offline** odaklı framework yok

→ **Steward'ın fırsatı:** Rust-native, local-first, graph-DB-backed agent OS

---

## 3. Graph DB & Memory Research — Özet

| Proje | Dil | Tür | Öne Çıkan |
|-------|-----|-----|-----------|
| **Neo4j agent-memory** | Python/TS | Client-server | 3 memory type (short/long/reasoning), MCP server |
| **Graphiti (Zep)** | Python | Client-server | Temporal KG, bi-temporal model, hybrid search |
| **Cortex** | **Rust** | **Embedded** | Self-organizing KG, auto-linking, briefing, tek binary |
| **Graphyne** | **Rust** | **Embedded + gRPC** | BM25+Vector+Graph hybrid, agent memory, gRPC native |
| **NeuroGraphRAG** | **Rust** | **Embedded (SQLite)** | fastembed + sqlite-vec + FTS5 hybrid, tek binary |
| **HybridMind** | Python | Embedded | FAISS + NetworkX + SQLite, `.mind` file |

### Steward İçin Seçim

**Kısa vadeli öneri: Rust-native graph katmanı** (Cortex/Graphyne tarzı)
- Var olan Rust stack'ine doğal uyum
- External DB (Neo4j) bağımlılığı yok
- `petgraph` + `sqlite-vec` ile embedded çözüm
- gRPC üzerinden servis edilir (mevcut altyapı)

**Orta vadeli: Neo4j connector** (opsiyonel, isteğe bağlı)
- Daha büyük/karmaşık graph'lar için
- Neo4j Labs agent-memory protokollerini implemente et

---

## 4. Yeni Mimarı

```
┌─────────────────────────────────────────────────────────────────┐
│                        STEWARD AGENT OS                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                     USER INTERFACES                      │   │
│  │  ┌───────────┐  ┌───────────┐  ┌────────────────────┐   │   │
│  │  │ CLI/TUI   │  │  Web UI   │  │  MCP Client/Server │   │   │
│  │  │ (ratatui) │  │ (Next.js) │  │  (Anthropic MCP)   │   │   │
│  │  └─────┬─────┘  └─────┬─────┘  └─────────┬──────────┘   │   │
│  └────────┼──────────────┼───────────────────┼──────────────┘   │
│           │              │                   │                  │
│           └──────────────┼───────────────────┘                  │
│                          │ gRPC / gRPC-web / MCP                │
│  ┌───────────────────────▼─────────────────────────────────┐   │
│  │              STEWARD DAEMON (Rust)                       │   │
│  │                                                          │   │
│  │  ┌──────────────────────────────────────────────────┐   │   │
│  │  │              SERVICE LAYER                       │   │   │
│  │  │  ┌──────────┐ ┌──────────┐ ┌────────────────┐   │   │   │
│  │  │  │ Ping     │ │ Execute  │ │ RunPlugin      │   │   │   │
│  │  │  │ (health) │ │ Task     │ │ (wasm sandbox) │   │   │   │
│  │  │  └──────────┘ └──────────┘ └────────────────┘   │   │   │
│  │  │  ┌──────────────────┐ ┌────────────────────┐    │   │   │
│  │  │  │ Workflow Service │ │ Agent Memory Svc   │    │   │   │
│  │  │  │ (orchestration)  │ │ (graph RAG)        │    │   │   │
│  │  │  └──────────────────┘ └────────────────────┘    │   │   │
│  │  └──────────────────────────────────────────────────┘   │   │
│  │                                                          │   │
│  │  ┌──────────────────────────────────────────────────┐   │   │
│  │  │              ENGINE LAYER                        │   │   │
│  │  │                                                  │   │   │
│  │  │  ┌──────────────────────────────────────────┐   │   │   │
│  │  │  │     WORKFLOW ENGINE                       │   │   │   │
│  │  │  │  ┌──────────┐ ┌──────────┐ ┌─────────┐  │   │   │   │
│  │  │  │  │ Discovery│ │ Context  │ │ Research│  │   │   │   │
│  │  │  │  │ (interview│ │ Reader   │ │ Agent   │  │   │   │   │
│  │  │  │  │  agent)   │ │          │ │         │  │   │   │   │
│  │  │  │  └──────────┘ └──────────┘ └─────────┘  │   │   │   │
│  │  │  │  ┌──────────┐ ┌──────────┐ ┌─────────┐  │   │   │   │
│  │  │  │  │ Intake   │ │Orchestra │ │ Plan-   │  │   │   │   │
│  │  │  │  │ (budget/ │ │tor Core  │ │ Execute │  │   │   │   │
│  │  │  │  │  router) │ │          │ │         │  │   │   │   │
│  │  │  │  └──────────┘ └──────────┘ └─────────┘  │   │   │   │
│  │  │  └──────────────────────────────────────────┘   │   │   │
│  │  │                                                  │   │   │
│  │  │  ┌──────────────────────────────────────────┐   │   │   │
│  │  │  │     KNOWLEDGE ENGINE                      │   │   │   │
│  │  │  │  ┌──────────┐ ┌──────────┐ ┌─────────┐  │   │   │   │
│  │  │  │  │ Vector   │ │ Graph    │ │ Hybrid  │  │   │   │   │
│  │  │  │  │ Store    │ │ Engine   │ │ RAG     │  │   │   │   │
│  │  │  │  │(sqlite-  │ │(petgraph)│ │(BM25+   │  │   │   │   │
│  │  │  │  │ vec)     │ │          │ │ vector+ │  │   │   │   │
│  │  │  │  │          │ │          │ │ graph)  │  │   │   │   │
│  │  │  │  └──────────┘ └──────────┘ └─────────┘  │   │   │   │
│  │  │  │  ┌────────────────────────────────────┐  │   │   │   │
│  │  │  │  │     AGENT MEMORY                   │  │   │   │   │
│  │  │  │  │  ┌────────────┐ ┌──────────────┐  │  │   │   │   │
│  │  │  │  │  │ Short-term │ │ Long-term    │  │  │   │   │   │
│  │  │  │  │  │ (session)  │ │ (persistent) │  │  │   │   │   │
│  │  │  │  │  └────────────┘ └──────────────┘  │  │   │   │   │
│  │  │  │  │  ┌─────────────────────────────┐  │  │   │   │   │
│  │  │  │  │  │ Reasoning Memory (traces)    │  │  │   │   │   │
│  │  │  │  │  └─────────────────────────────┘  │  │   │   │   │
│  │  │  │  └────────────────────────────────────┘  │   │   │   │
│  │  │  └──────────────────────────────────────────┘   │   │   │
│  │  │                                                  │   │   │
│  │  │  ┌──────────────────────────────────────────┐   │   │   │
│  │  │  │     PLUGIN SANDBOX                        │   │   │   │
│  │  │  │  ┌──────────┐ ┌──────────┐ ┌─────────┐  │   │   │   │
│  │  │  │  │ Wasmtime │ │ WASI     │ │ Plugin  │  │   │   │   │
│  │  │  │  │ Runtime  │ │ Host     │ │ Registry│  │   │   │   │
│  │  │  │  └──────────┘ └──────────┘ └─────────┘  │   │   │   │
│  │  │  └──────────────────────────────────────────┘   │   │   │
│  │  └──────────────────────────────────────────────────┘   │   │
│  │                                                          │   │
│  │  ┌──────────────────────────────────────────────────┐   │   │
│  │  │              STORAGE LAYER                       │   │   │
│  │  │  ┌──────────┐ ┌──────────┐ ┌────────────────┐   │   │   │
│  │  │  │ SQLite   │ │ Graph DB │ │ File Store     │   │   │   │
│  │  │  │ (tasks,  │ │ (memory, │ │ (artifacts,    │   │   │   │
│  │  │  │  config) │ │  KG)     │ │  plugin files) │   │   │   │
│  │  │  └──────────┘ └──────────┘ └────────────────┘   │   │   │
│  │  └──────────────────────────────────────────────────┘   │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │           MULTI-AGENT DEVELOPMENT SYSTEM (.agents/)     │   │
│  │  AI agents that build / maintain Steward itself          │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 5. Yeni Workflow Engine (Kullanıcının İstediği Flowchart)

Kullanıcının verdiği mermaid flowchart, Steward'ın **Workflow Engine**'inin ta kendisi olacak:

```
USER TASK
  │
  ▼
┌──────────────────────────────────────────────────────────────┐
│  DISCOVERY — Deep Interview                                  │
│  • Deep Interview Agent: Kullanıcıyı sorguya çeker           │
│  • Scope Clarifier: In/out-of-scope, non-goals, risk         │
│  • Acceptance Criteria Builder: Done tanımı, test beklentisi │
└──────────────────────────────────────────────────────────────┘
  │
  ▼
┌──────────────────────────────────────────────────────────────┐
│  CONTEXT — Project Scope Reader                              │
│  • Project Scope Scanner: Repo yapısı, README, config        │
│  • Codebase Mapper: Modüller, entry point'ler, bağımlılıklar │
│  • Constraint Extractor: Framework seçimleri, security       │
└──────────────────────────────────────────────────────────────┘
  │
  ▼
┌──────────────────────────────────────────────────────────────┐
│  RESEARCH — Best-Practice Lookup                             │
│  • Research Agent: Dokümanlar, repo örnekleri                │
│  • Source Quality Filter: Official docs first                │
│  • Implementation Strategy Synthesis                         │
└──────────────────────────────────────────────────────────────┘
  │
  ▼
┌──────────────────────────────────────────────────────────────┐
│  INTAKE — Resource Management                                │
│  • Token Budget Manager: Per-task hard limit                 │
│  • Model Router: Fast / Balanced / Strong                    │
│  • Graceful Degradation: Fallback, cache, budget downgrade  │
└──────────────────────────────────────────────────────────────┘
  │
  ▼
┌──────────────────────────────────────────────────────────────┐
│  ORCHESTRATOR — Supervisor Core                              │
│  • Task decomposition, agent assignment, priority queue      │
│  • Behavioral Contract Registry (role contracts)             │
│  • Circuit Breaker (3x fail → pause, loop detection)        │
└──────────────────────────────────────────────────────────────┘
  │
  ▼
┌──────────────────────────────────────────────────────────────┐
│  PLAN — Plan-Then-Execute                                    │
│  • Plan Generator: Steps + dependency graph, cost estimate   │
│  • Plan Validator: Cycle check, budget, security             │
│  • Workflow Recommender: Parallel / Sequential / Hybrid      │
└──────────────────────────────────────────────────────────────┘
  │
  ▼
┌──────────────────────────────────────────────────────────────┐
│  USER CHOICE — Parallel / Sequential / Hybrid                │
└──────────────────────────────────────────────────────────────┘
  │
  ▼
┌──────────────────────────────────────────────────────────────┐
│  EXECUTION — Multi-Agent Pipeline                            │
│  Parallel: Arch + Design + Backend + Frontend + DevOps + CLI │
│  • Isolated Git Worktrees                                    │
│  • Map-Reduce Execution                                      │
│  • Dynamic Task Agent (spawned when needed)                  │
└──────────────────────────────────────────────────────────────┘
  │
  ▼
┌──────────────────────────────────────────────────────────────┐
│  VALIDATION — Quality Gates                                  │
│  • Test Agent: Integration + E2E tests                       │
│  • Review Agent: Code review, diff analysis                  │
│  • Victory Verifier: Acceptance criteria audit               │
└──────────────────────────────────────────────────────────────┘
```

---

## 6. Protobuf Tanımları (Yeni RPC'ler)

```protobuf
syntax = "proto3";
package steward;

service StewardService {
  // Mevcut
  rpc Ping (PingRequest) returns (PingResponse);
  rpc RunPlugin (RunPluginRequest) returns (RunPluginResponse);
  rpc ExecuteTask (ExecuteTaskRequest) returns (ExecuteTaskResponse);

  // YENİ — Workflow Engine
  rpc StartWorkflow (StartWorkflowRequest) returns (stream WorkflowEvent);
  rpc GetWorkflowStatus (GetWorkflowStatusRequest) returns (WorkflowStatus);
  rpc ListWorkflows (ListWorkflowsRequest) returns (ListWorkflowsResponse);
  rpc CancelWorkflow (CancelWorkflowRequest) returns (CancelWorkflowResponse);
  rpc ApprovePlan (ApprovePlanRequest) returns (ApprovePlanResponse);

  // YENİ — Knowledge Graph / Agent Memory
  rpc StoreMemory (StoreMemoryRequest) returns (StoreMemoryResponse);
  rpc RecallMemory (RecallMemoryRequest) returns (RecallMemoryResponse);
  rpc GraphQuery (GraphQueryRequest) returns (GraphQueryResponse);
  rpc GetKnowledgeGraph (GetKnowledgeGraphRequest) returns (GetKnowledgeGraphResponse);

  // YENİ — Agent Management
  rpc ListAgents (ListAgentsRequest) returns (ListAgentsResponse);
  rpc GetAgentLog (GetAgentLogRequest) returns (stream AgentLogEntry);
}
```

---

## 7. Implementasyon Planı (Fazlar)

### Faz 1 — Knowledge Engine (Önce altyapı)
- SQLite + sqlite-vec'i knowledge graph olarak genişlet
- `petgraph` ile in-memory graph traversal
- Entity extraction, relationship storage
- Hybrid search (BM25 + vector + graph) — RRF fusion
- Protobuf: `StoreMemory`, `RecallMemory`, `GraphQuery`

### Faz 2 — Workflow Engine (Çekirdek)
- Workflow state machine (Discovery → Context → Research → ...)
- Deep Interview Agent implementasyonu
- Scope / Acceptance criteria modülleri
- Plan Generator + Validator
- Circuit Breaker + Behavioral Contracts
- Protobuf: `StartWorkflow`, streaming events

### Faz 3 — Execution Pipeline
- Parallel agent execution (fan-out/fan-in)
- Git worktree isolation
- Map-Reduce pattern
- Dynamic task agent spawning
- Protobuf: execution RPC'leri

### Faz 4 — Web UI (Dashboard)
- Workflow yönetimi (başlat/izle/iptal)
- Knowledge graph görselleştirme (D3.js / vis.js)
- Agent log viewer (streaming)
- Deep Interview arayüzü (soru-cevap)
- Plan onay/red UI

### Faz 5 — CLI/TUI Enhancement
- Interaktif workflow modu
- Dashboard bilgileri
- Graph query CLI

---

## 8. Rust Crate Yapısı (Yeni)

```
steward/
├── crates/
│   ├── core/           # Mevcut — protobuf tipleri
│   ├── daemon/         # Mevcut — gRPC server
│   ├── cli/            # Mevcut — CLI/TUI
│   ├── knowledge/      # YENİ — Knowledge Engine
│   │   ├── src/
│   │   │   ├── graph/       # petgraph wrapper, traversal
│   │   │   ├── vector/      # sqlite-vec wrapper
│   │   │   ├── hybrid/      # RRF fusion, ranking
│   │   │   ├── memory/      # Agent memory (short/long/reasoning)
│   │   │   └── extraction/  # Entity extraction
│   │   └── Cargo.toml
│   ├── workflow/       # YENİ — Workflow Engine
│   │   ├── src/
│   │   │   ├── discovery/   # Deep Interview, Scope, Acceptance
│   │   │   ├── context/     # Project scope reader
│   │   │   ├── research/    # Research agent
│   │   │   ├── intake/      # Budget, Router, Degradation
│   │   │   ├── orchestrator/ # Supervisor, Circuit Breaker
│   │   │   ├── plan/        # Plan Generator, Validator
│   │   │   └── execution/   # Pipeline execution
│   │   └── Cargo.toml
│   └── integration-tests/ # Mevcut
├── proto/
│   └── steward.proto      # Genişletilecek
└── web-ui/                # Mevcut — Next.js, geliştirilecek
```

---

## 9. Seçim Gerekçeleri

### Neden Rust-native graph (Neo4j değil)?
- **Mevcut stack'e uyum**: Zaten Rust + SQLite + gRPC
- **Zero external dependency**: Kullanıcının Neo4j kurması gerekmez
- **Performance**: petgraph in-memory traversal nanosecond seviyesinde
- **Portability**: Tek binary, her yerde çalışır
- **Opsiyonel Neo4j**: İleride adapter olarak eklenebilir

### Neden petgraph + sqlite-vec?
- `petgraph`: Rust'ın en olgun graph kütüphanesi, stable, iyi test edilmiş
- `sqlite-vec`: Zaten projede var, vector search için yeterli
- İkisi birlikte hybrid search (BM25 + vector + graph) için ideal

### Neden gRPC streaming (workflow events)?
- Kullanıcı workflow ilerlemesini canlı görmeli
- Web UI için Server-Sent Events benzeri pattern
- Agent log'ları anlık akmalı

---

## 10. Komşu Projelerden Alınacak Dersler

| Proje | Alınacak Ders |
|-------|---------------|
| **Cortex** | Self-organizing graph, auto-linking, briefing → memory pattern'ı |
| **Graphyne** | gRPC native servis, hybrid scoring → API tasarımı |
| **Neo4j agent-memory** | 3 memory type modeli (short/long/reasoning) |
| **Graphiti** | Bi-temporal model, conflict resolution → ileri faz |
| **LangGraph** | Checkpointing, interrupt/resume pattern'ı |
| **CrewAI** | Role-based agent tanımı → Behavioral contracts |

---

## Next Steps

1. ~~Research complete~~ ✅
2. Architecture design complete ✅
3. **Faz 1: Knowledge Engine** — implementasyon başlasın
4. **Faz 2: Workflow Engine** — state machine + discovery agent
5. **Faz 4: Web UI** — dashboard (paralel ilerleyebilir)
