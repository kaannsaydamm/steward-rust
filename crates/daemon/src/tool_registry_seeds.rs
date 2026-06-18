use super::{RiskLevel, ToolRuntime};

pub(super) struct ToolSeed {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub runtime: ToolRuntime,
    pub risk: RiskLevel,
    pub enabled: bool,
    pub requires_approval: bool,
}

pub(super) struct SkillSeed {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub enabled: bool,
    pub tools: &'static [&'static str],
}

pub(super) const TOOLS: &[ToolSeed] = &[
    ToolSeed {
        id: "fs.read",
        name: "Read files",
        description: "Read workspace files within policy roots",
        runtime: ToolRuntime::Builtin,
        risk: RiskLevel::Low,
        enabled: false,
        requires_approval: false,
    },
    ToolSeed {
        id: "fs.search",
        name: "Search files",
        description: "Search workspace paths and text",
        runtime: ToolRuntime::Builtin,
        risk: RiskLevel::Low,
        enabled: false,
        requires_approval: false,
    },
    ToolSeed {
        id: "memory.recall",
        name: "Recall memory",
        description: "Query Steward relational and vector memory",
        runtime: ToolRuntime::Builtin,
        risk: RiskLevel::Low,
        enabled: true,
        requires_approval: false,
    },
    ToolSeed {
        id: "memory.store",
        name: "Store memory",
        description: "Persist operator-approved memory and lessons",
        runtime: ToolRuntime::Builtin,
        risk: RiskLevel::Medium,
        enabled: true,
        requires_approval: true,
    },
    ToolSeed {
        id: "process.exec",
        name: "Execute process",
        description: "Run an allowlisted local process",
        runtime: ToolRuntime::Process,
        risk: RiskLevel::High,
        enabled: false,
        requires_approval: true,
    },
    ToolSeed {
        id: "wasm.run",
        name: "Run WASM",
        description: "Execute a sandboxed WASM module",
        runtime: ToolRuntime::Wasm,
        risk: RiskLevel::High,
        enabled: false,
        requires_approval: true,
    },
    ToolSeed {
        id: "workflow.inspect",
        name: "Inspect workflow",
        description: "Read workflow state and agent logs",
        runtime: ToolRuntime::Builtin,
        risk: RiskLevel::Low,
        enabled: true,
        requires_approval: false,
    },
    ToolSeed {
        id: "workflow.manage",
        name: "Manage workflow",
        description: "Start, approve, or cancel workflows",
        runtime: ToolRuntime::Builtin,
        risk: RiskLevel::High,
        enabled: false,
        requires_approval: true,
    },
];

pub(super) const SKILLS: &[SkillSeed] = &[
    SkillSeed {
        id: "codebase-research",
        name: "Codebase research",
        description: "Inspect a workspace and recall relevant context",
        enabled: false,
        tools: &["fs.search", "fs.read", "memory.recall"],
    },
    SkillSeed {
        id: "reflective-memory",
        name: "Reflective memory",
        description: "Recall context and store durable lessons",
        enabled: true,
        tools: &["memory.recall", "memory.store"],
    },
    SkillSeed {
        id: "workflow-operator",
        name: "Workflow operator",
        description: "Inspect and govern durable workflows",
        enabled: false,
        tools: &["workflow.inspect", "workflow.manage"],
    },
];
