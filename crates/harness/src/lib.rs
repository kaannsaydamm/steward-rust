//! `steward-harness`: inspectable, versionable agent behavior (Omega §26,
//! §29, §30).

pub mod agent_profile;
pub mod agents;

pub use agent_profile::{AgentProfile, ContextMode, ModelPolicy, ProfileError, SpawnMode};
pub use agents::scheduler::{SpawnRequest, SubagentScheduler};
pub use agents::task::AsyncTask;
