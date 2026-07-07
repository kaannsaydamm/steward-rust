//! Canned prompt templates for `/init`, `/interview`, and `/deepwork` — these are shortcuts
//! that frame an ordinary chat turn, not a distinct runtime mode. They exist because typing
//! the same framing out by hand every time is tedious, matching how comparable tools (Claude
//! Code, OpenCode, Hermes) expose them as slash commands rather than special execution paths.

pub const INIT: &str = "\
Explore this workspace using your governed tools (fs.search, fs.read, git.diff). Identify the \
tech stack, directory layout, build/test/lint commands, and coding conventions. Then write a \
concise project-context summary I can reuse to onboard a future session quickly.";

pub fn interview(topic: &str) -> String {
    let subject = if topic.is_empty() {
        "the task ahead".to_owned()
    } else {
        topic.to_owned()
    };
    format!(
        "Before you start any work, interview me about {subject}: ask clarifying questions \
about scope, constraints, success criteria, and priorities. Ask one focused question at a \
time and wait for my answer before asking the next. Do not start implementing until we've \
covered the essentials."
    )
}

pub fn deepwork(task: &str) -> String {
    format!(
        "Work autonomously and end-to-end on the following task. Use governed tools as \
needed, keep going through multiple steps without stopping to check in, and only pause if \
you hit a decision that truly requires my input. Report back with a summary when the task is \
done or you're genuinely blocked.\n\nTask: {task}"
    )
}

#[cfg(test)]
mod tests {
    use super::{deepwork, interview, INIT};

    #[test]
    fn init_prompt_is_non_empty() {
        assert!(!INIT.is_empty());
    }

    #[test]
    fn interview_defaults_the_subject_when_topic_is_empty() {
        assert!(interview("").contains("the task ahead"));
    }

    #[test]
    fn interview_uses_the_given_topic() {
        assert!(interview("the payments migration").contains("the payments migration"));
    }

    #[test]
    fn deepwork_embeds_the_task_text() {
        assert!(deepwork("ship the release notes").contains("ship the release notes"));
    }
}
