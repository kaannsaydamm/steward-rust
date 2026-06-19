use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "steward",
    author,
    version,
    about = "Steward Agent OS interactive operator CLI",
    long_about = "Steward connects to the local daemon and opens the interactive Butler operator shell when no subcommand is provided."
)]
pub struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:50051", global = true)]
    pub host: String,

    #[arg(long, default_value_t = false, global = true)]
    pub no_auto_start: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Ping,
    Status,
    Doctor(DoctorArgs),
    Task(TaskArgs),
    Workflow(WorkflowArgs),
    Memory(MemoryArgs),
    Tools(ToolsArgs),
    Skills(SkillsArgs),
    Data(DataArgs),
    Mcp(McpArgs),
    Maintenance(MaintenanceArgs),
    Agents,
}

#[derive(Debug, Args)]
pub struct MaintenanceArgs {
    #[command(subcommand)]
    pub command: MaintenanceCommand,
}

#[derive(Debug, Subcommand)]
pub enum MaintenanceCommand {
    Status,
    Prune,
}

#[derive(Debug, Args)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: McpCommand,
}

#[derive(Debug, Subcommand)]
pub enum McpCommand {
    Add(McpAddArgs),
    List,
    Start(McpIdArgs),
    Stop(McpIdArgs),
    Remove(McpIdArgs),
}

#[derive(Debug, Args)]
pub struct McpAddArgs {
    pub adapter_id: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub cwd: Option<PathBuf>,
    pub command: String,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub arguments: Vec<String>,
}

#[derive(Debug, Args)]
pub struct McpIdArgs {
    pub adapter_id: String,
}

#[derive(Debug, Args)]
pub struct DataArgs {
    #[command(subcommand)]
    pub command: DataCommand,
}

#[derive(Debug, Subcommand)]
pub enum DataCommand {
    Export(DataPathArgs),
    Import(DataPathArgs),
}

#[derive(Debug, Args)]
pub struct DataPathArgs {
    pub archive: PathBuf,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long, default_value_t = false)]
    pub strict: bool,
}

#[derive(Debug, Args)]
pub struct ToolsArgs {
    #[command(subcommand)]
    pub command: ToolCommand,
}

#[derive(Debug, Subcommand)]
pub enum ToolCommand {
    List,
    Invoke(ToolInvokeArgs),
    History(ToolHistoryArgs),
}

#[derive(Debug, Args)]
pub struct ToolInvokeArgs {
    pub tool_id: String,

    #[arg(long = "arg", value_name = "KEY=VALUE")]
    pub arguments: Vec<String>,

    #[arg(long, default_value_t = false)]
    pub approve: bool,
}

#[derive(Debug, Args)]
pub struct ToolHistoryArgs {
    #[arg(long, default_value_t = 20)]
    pub limit: i32,
}

#[derive(Debug, Args)]
pub struct SkillsArgs {
    #[command(subcommand)]
    pub command: SkillCommand,
}

#[derive(Debug, Subcommand)]
pub enum SkillCommand {
    List,
    Install(SkillInstallArgs),
}

#[derive(Debug, Args)]
pub struct SkillInstallArgs {
    pub bundle: PathBuf,
}

#[derive(Debug, Args)]
pub struct TaskArgs {
    #[arg(required = true)]
    pub text: Vec<String>,
}

#[derive(Debug, Args)]
pub struct WorkflowArgs {
    #[command(subcommand)]
    pub command: WorkflowCommand,
}

#[derive(Debug, Args)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    Remember(MemoryRememberArgs),
    Lesson(MemoryLessonArgs),
    Recall(MemoryRecallArgs),
    Dreams(MemoryRecallArgs),
}

#[derive(Debug, Args)]
pub struct MemoryRememberArgs {
    #[arg(required = true)]
    pub text: Vec<String>,

    #[arg(long, value_enum, default_value_t = MemoryKind::Long)]
    pub kind: MemoryKind,
}

#[derive(Debug, Args)]
pub struct MemoryLessonArgs {
    #[arg(long)]
    pub scope: String,

    #[arg(long)]
    pub error: String,

    #[arg(long)]
    pub correction: String,

    #[arg(long, default_value_t = 3)]
    pub severity: i32,
}

#[derive(Debug, Args)]
pub struct MemoryRecallArgs {
    #[arg(default_value = "")]
    pub query: String,

    #[arg(long, default_value_t = 10)]
    pub limit: i32,
}

#[derive(Debug, Subcommand)]
pub enum WorkflowCommand {
    Start(WorkflowStartArgs),
    List,
    Status(WorkflowStatusArgs),
    Watch(WorkflowWatchArgs),
    Logs(WorkflowLogsArgs),
    Approve(WorkflowApproveArgs),
    Cancel(WorkflowCancelArgs),
}

#[derive(Debug, Args)]
pub struct WorkflowStartArgs {
    #[arg(required = true)]
    pub title: Vec<String>,

    #[arg(long, default_value = "")]
    pub description: String,

    #[arg(long, default_value = "")]
    pub repo: String,
}

#[derive(Debug, Args)]
pub struct WorkflowApproveArgs {
    pub workflow_id: String,

    #[arg(long, value_enum, default_value_t = WorkflowMode::Hybrid)]
    pub mode: WorkflowMode,
}

#[derive(Debug, Args)]
pub struct WorkflowStatusArgs {
    pub workflow_id: String,
}

#[derive(Debug, Args)]
pub struct WorkflowWatchArgs {
    pub workflow_id: String,

    #[arg(long, default_value_t = 500)]
    pub interval_ms: u64,

    #[arg(long, default_value_t = 0)]
    pub max_ticks: usize,

    #[arg(long, default_value_t = false)]
    pub keep_waiting_for_approval: bool,
}

#[derive(Debug, Args)]
pub struct WorkflowLogsArgs {
    pub workflow_id: String,
    pub agent_id: String,

    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct WorkflowCancelArgs {
    pub workflow_id: String,

    #[arg(long, default_value = "cancelled from steward cli")]
    pub reason: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum WorkflowMode {
    Parallel,
    Sequential,
    Hybrid,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum MemoryKind {
    Short,
    Long,
    Reasoning,
    Negative,
    Dream,
}

impl MemoryKind {
    pub const fn code(self) -> i32 {
        match self {
            Self::Short => 0,
            Self::Long => 1,
            Self::Reasoning => 2,
            Self::Negative => 3,
            Self::Dream => 4,
        }
    }
}

impl WorkflowMode {
    pub const fn code(self) -> i32 {
        match self {
            Self::Parallel => 0,
            Self::Sequential => 1,
            Self::Hybrid => 2,
        }
    }
}
