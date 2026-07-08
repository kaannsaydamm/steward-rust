use crate::provider_commands::ProviderArgs;
use crate::session_commands::SessionArgs;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

const AFTER_HELP: &str = "\
Examples:
    steward                          Start interactive chat (auto-starts the daemon if needed)
    steward -z \"summarize main.rs\"    One-shot: print only the final response, no banner/spinner
    steward dashboard                 Start the daemon if needed and open the web UI in a browser
    steward dashboard --status        Show whether the daemon/web UI is running
    steward dashboard --stop          Stop the local daemon
    steward doctor                    Inspect local runtime health
    steward provider list             List configured provider profiles
    steward cron list                 List scheduled cron jobs
    steward logs                      View the daemon log (last 50 lines)
    steward logs -f                   Follow the daemon log in real time
    steward completion powershell     Print a shell completion script";

#[derive(Debug, Parser)]
#[command(
    name = "steward",
    author,
    version,
    about = "Steward Agent OS interactive operator CLI",
    long_about = "Steward connects to the local daemon and opens the interactive Butler operator shell when no subcommand is provided.",
    after_help = AFTER_HELP
)]
pub struct Cli {
    /// Daemon gRPC endpoint to connect to (default: from ~/.steward settings, usually
    /// http://127.0.0.1:50051).
    #[arg(long, global = true)]
    pub host: Option<String>,

    /// Never auto-start the local daemon; fail instead if it isn't already running.
    #[arg(long, default_value_t = false, global = true)]
    pub no_auto_start: bool,

    /// One-shot mode: send a single prompt and print ONLY the final response text to stdout.
    /// No banner, no spinner, no tool previews. Governed tools still run; approvals are
    /// auto-bypassed. Intended for scripts/pipes.
    #[arg(short = 'z', long = "oneshot", value_name = "PROMPT")]
    pub oneshot: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check whether the daemon is reachable
    Ping,
    /// Compact operator status: daemon, agents, workflows
    Status,
    /// Inspect local runtime health (config, ports, provider, storage)
    Doctor(DoctorArgs),
    /// Store an explicit task without invoking the model
    Task(TaskArgs),
    /// Start, list, inspect, watch, approve, or cancel workflows
    Workflow(WorkflowArgs),
    /// Store and recall long-term memory, lessons, and nightly dreams
    Memory(MemoryArgs),
    /// List and enable/disable governed tools
    Tools(ToolsArgs),
    /// List and install signed skill packs
    Skills(SkillsArgs),
    /// Export or import the entire ~/.steward home directory
    Data(DataArgs),
    /// Register, start, stop, and remove MCP adapters
    Mcp(McpArgs),
    /// Show or run retention/pruning maintenance
    Maintenance(MaintenanceArgs),
    /// List registered agents
    Agents,
    /// Interactive setup wizard for daemon/provider settings
    Setup(SetupArgs),
    /// Manage model provider profiles (list, save, activate, remove)
    Provider(ProviderArgs),
    /// Manage saved chat sessions
    Session(SessionArgs),
    /// Manage the process.exec command allowlist
    Security(SecurityArgs),
    /// Schedule a governed tool to run on a recurring interval
    Cron(CronArgs),
    /// Start the daemon and open the web UI, or check/stop it
    Dashboard(DashboardArgs),
    /// View or follow the daemon log
    Logs(LogsArgs),
    /// Print a shell completion script (bash, zsh, fish, powershell)
    Completion(CompletionArgs),
    /// Save, browse, and remove artifacts (code, text, links, diffs) produced by sessions
    Artifact(ArtifactArgs),
    /// Show uncommitted changes in the workspace's git repository
    Diff(DiffArgs),
    /// Create and check out a git branch in the workspace
    Branch(BranchArgs),
    /// Bridge external chat channels (Telegram bot) to the local agent
    Channel(ChannelArgs),
}

#[derive(Debug, Args)]
pub struct ChannelArgs {
    #[command(subcommand)]
    pub command: ChannelCommand,
}

#[derive(Debug, Subcommand)]
pub enum ChannelCommand {
    /// Show bridge configuration (token presence and allowed chats)
    Status,
    /// Save the Telegram bot token (restart the daemon to start the bridge)
    SetTelegram(ChannelTokenArgs),
    /// Remove the Telegram bot token and stop bridging on next restart
    ClearTelegram,
    /// Allow a Telegram chat id to talk to the agent (takes effect immediately)
    Allow(ChannelChatArgs),
    /// Remove a Telegram chat id from the allowlist
    Disallow(ChannelChatArgs),
}

#[derive(Debug, Args)]
pub struct ChannelTokenArgs {
    pub token: String,
}

#[derive(Debug, Args)]
pub struct ChannelChatArgs {
    pub chat_id: i64,
}

#[derive(Debug, Args)]
pub struct DiffArgs {
    /// Limit the diff to this file or directory.
    #[arg(default_value = "")]
    pub path: String,

    /// Diff staged changes instead of the working tree.
    #[arg(long)]
    pub staged: bool,
}

#[derive(Debug, Args)]
pub struct BranchArgs {
    pub name: String,

    /// Check out an existing branch instead of creating a new one.
    #[arg(long)]
    pub existing: bool,
}

#[derive(Debug, Args)]
pub struct ArtifactArgs {
    #[command(subcommand)]
    pub command: ArtifactCommand,
}

#[derive(Debug, Subcommand)]
pub enum ArtifactCommand {
    /// List artifacts, optionally filtered by a search term
    List(ArtifactListArgs),
    /// Save a new artifact (reads content from stdin unless --content is given)
    Create(ArtifactCreateArgs),
    /// Print a single artifact's full content
    Show(ArtifactIdArgs),
    /// Remove an artifact
    Delete(ArtifactIdArgs),
}

#[derive(Debug, Args)]
pub struct ArtifactListArgs {
    /// Only show artifacts whose title or content contains this text
    #[arg(default_value = "")]
    pub query: String,
}

#[derive(Debug, Args)]
pub struct ArtifactCreateArgs {
    pub title: String,
    #[arg(value_enum)]
    pub kind: ArtifactKindArg,
    /// Inline content. If omitted, content is read from stdin.
    #[arg(long)]
    pub content: Option<String>,
    /// Language hint for code artifacts (e.g. "rust", "html").
    #[arg(long, default_value = "")]
    pub language: String,
    /// Chat session this artifact came from, if any.
    #[arg(long, default_value = "")]
    pub session_id: String,
}

#[derive(Debug, Args)]
pub struct ArtifactIdArgs {
    pub artifact_id: String,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ArtifactKindArg {
    Code,
    Text,
    Markdown,
    Image,
    Link,
    Diff,
}

#[derive(Debug, Args)]
pub struct DashboardArgs {
    /// Stop the running local daemon instead of starting it.
    #[arg(long, conflicts_with = "status")]
    pub stop: bool,

    /// Report whether the daemon/web UI is currently running, without starting or stopping it.
    #[arg(long)]
    pub status: bool,
}

#[derive(Debug, Args)]
pub struct LogsArgs {
    /// Follow the log file in real time (like `tail -f`).
    #[arg(short = 'f', long)]
    pub follow: bool,

    /// Only show lines from the last DURATION (e.g. "1h", "30m", "10s").
    #[arg(long, value_name = "DURATION")]
    pub since: Option<String>,

    /// Number of trailing lines to print when not following.
    #[arg(long, default_value_t = 50)]
    pub lines: usize,
}

#[derive(Debug, Args)]
pub struct CompletionArgs {
    pub shell: clap_complete::Shell,
}

#[derive(Debug, Args)]
pub struct CronArgs {
    #[command(subcommand)]
    pub command: CronCommand,
}

#[derive(Debug, Subcommand)]
pub enum CronCommand {
    List,
    Create(CronCreateArgs),
    Enable(CronJobIdArgs),
    Disable(CronJobIdArgs),
    Delete(CronJobIdArgs),
    Run(CronJobIdArgs),
}

#[derive(Debug, Args)]
pub struct CronCreateArgs {
    pub name: String,
    pub tool_id: String,
    /// Interval between runs in seconds (minimum 60).
    pub interval_seconds: i64,
    /// JSON object of tool arguments, e.g. '{"query":"hello"}'.
    #[arg(long, default_value = "{}")]
    pub input_json: String,
}

#[derive(Debug, Args)]
pub struct CronJobIdArgs {
    pub job_id: String,
}

#[derive(Debug, Args)]
pub struct SecurityArgs {
    #[command(subcommand)]
    pub command: SecurityCommand,
}

#[derive(Debug, Subcommand)]
pub enum SecurityCommand {
    List,
    Allow(SecurityProgramArgs),
    Disallow(SecurityProgramArgs),
}

#[derive(Debug, Args)]
pub struct SecurityProgramArgs {
    pub program: String,
}

#[derive(Debug, Args)]
pub struct SetupArgs {
    #[arg(long, default_value_t = false)]
    pub quick: bool,

    #[arg(long)]
    pub web_port: Option<u16>,

    #[arg(long)]
    pub provider: Option<String>,

    #[arg(long)]
    pub model: Option<String>,

    #[arg(long)]
    pub base_url: Option<String>,

    #[arg(long)]
    pub api_key_env: Option<String>,

    #[arg(long, default_value_t = false)]
    pub skip_provider: bool,
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
    Catalog,
    /// Live-search Smithery's public connector registry (registry.smithery.ai).
    SearchMarketplace(MarketplaceQueryArgs),
    QuickAdd(McpQuickAddArgs),
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

/// Registers an adapter straight from the curated catalog (`steward mcp catalog`), skipping
/// having to retype its command/args by hand — matching a marketplace "quick add" affordance.
#[derive(Debug, Args)]
pub struct McpQuickAddArgs {
    pub catalog_id: String,
    pub adapter_id: String,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra_args: Vec<String>,
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
    Enable(ToolIdArgs),
    Disable(ToolIdArgs),
}

#[derive(Debug, Args)]
pub struct ToolIdArgs {
    pub tool_id: String,
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
    /// Live-search ClawHub's public skill marketplace (clawhub.ai).
    SearchMarketplace(MarketplaceQueryArgs),
    /// Save a ClawHub skill's full SKILL.md content as a local artifact.
    InstallMarketplace(SkillMarketplaceInstallArgs),
}

#[derive(Debug, Args)]
pub struct SkillInstallArgs {
    pub bundle: PathBuf,
}

#[derive(Debug, Args)]
pub struct MarketplaceQueryArgs {
    #[arg(default_value = "")]
    pub query: String,
}

#[derive(Debug, Args)]
pub struct SkillMarketplaceInstallArgs {
    pub slug: String,
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
