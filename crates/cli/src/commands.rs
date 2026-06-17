use crate::cli::{Command, MemoryCommand, WorkflowCommand};
use crate::client;
use crate::operator_status;
use crate::workflow_view;
use anyhow::Result;

pub async fn run(host: &str, command: Command) -> Result<()> {
    match command {
        Command::Ping => {
            let status = client::ping(host).await?;
            println!("daemon: {status}");
        }
        Command::Status => {
            let status = operator_status::load(host).await?;
            for line in status.lines() {
                println!("{line}");
            }
        }
        Command::Task(args) => {
            let task = args.text.join(" ");
            let status = client::execute_task(host, &task).await?;
            println!("{status}");
        }
        Command::Workflow(args) => run_workflow(host, args.command).await?,
        Command::Memory(args) => run_memory(host, args.command).await?,
        Command::Agents => {
            let agents = client::list_agents(host).await?;
            if agents.is_empty() {
                println!("no agents registered");
            }
            for agent in agents {
                println!(
                    "{}\t{}\t{}\t{:.0}%",
                    agent.agent_id,
                    agent.name,
                    agent.status,
                    agent.progress * 100.0
                );
            }
        }
    }
    Ok(())
}

async fn run_memory(host: &str, command: MemoryCommand) -> Result<()> {
    match command {
        MemoryCommand::Remember(args) => {
            let content = args.text.join(" ");
            let id = client::store_memory(host, args.kind.code(), &content, Vec::new(), Vec::new())
                .await?;
            println!("remembered\t{id}");
        }
        MemoryCommand::Lesson(args) => {
            let severity = args.severity.clamp(1, 5).to_string();
            let content = format!(
                "Mistake in {}: {}. Correction: {}",
                args.scope, args.error, args.correction
            );
            let id = client::store_memory(
                host,
                3,
                &content,
                vec![
                    ("scope".to_owned(), args.scope.clone()),
                    ("error_signature".to_owned(), args.error),
                    ("correction".to_owned(), args.correction),
                    ("severity".to_owned(), severity),
                ],
                vec![args.scope],
            )
            .await?;
            println!("lesson\t{id}");
        }
        MemoryCommand::Recall(args) => print_memories(host, &args.query, 0, args.limit).await?,
        MemoryCommand::Dreams(args) => print_memories(host, &args.query, 4, args.limit).await?,
    }
    Ok(())
}

async fn print_memories(host: &str, query: &str, memory_type: i32, limit: i32) -> Result<()> {
    let memories = client::recall_memory(host, query, memory_type, limit).await?;
    if memories.is_empty() {
        println!("no memories");
    }
    for memory in memories {
        println!(
            "{}\ttype={}\t{}",
            memory.id,
            memory.memory_type,
            memory.content.replace('\n', " ")
        );
    }
    Ok(())
}

async fn run_workflow(host: &str, command: WorkflowCommand) -> Result<()> {
    match command {
        WorkflowCommand::Start(args) => {
            let title = args.title.join(" ");
            let description = if args.description.is_empty() {
                title.as_str()
            } else {
                args.description.as_str()
            };
            let events = client::start_workflow(host, &title, description, &args.repo).await?;
            for event in events {
                println!(
                    "{}\t{}\t{}\t{:.0}%",
                    event.workflow_id, event.phase, event.message, event.progress
                );
                if event.requires_approval {
                    println!(
                        "approval required: steward workflow approve {}",
                        event.workflow_id
                    );
                }
            }
        }
        WorkflowCommand::List => {
            let workflows = client::list_workflows(host).await?;
            if workflows.is_empty() {
                println!("no workflows");
            }
            for workflow in workflows {
                println!(
                    "{}\t{}\tphase={}\tmode={}\t{:.0}%\t{}",
                    workflow.workflow_id,
                    workflow.title,
                    workflow.phase,
                    workflow.mode,
                    workflow.overall_progress,
                    workflow.status_message
                );
            }
        }
        WorkflowCommand::Status(args) => {
            let status = client::get_workflow_status(host, &args.workflow_id).await?;
            for line in workflow_view::status_lines(&status) {
                println!("{line}");
            }
        }
        WorkflowCommand::Logs(args) => {
            let logs =
                client::get_agent_logs(host, &args.workflow_id, &args.agent_id, args.limit).await?;
            if logs.is_empty() {
                println!("no logs");
            }
            for entry in logs {
                println!("{}", workflow_view::log_line(&entry));
            }
        }
        WorkflowCommand::Approve(args) => {
            let message =
                client::approve_workflow(host, &args.workflow_id, args.mode.code()).await?;
            println!("{message}");
        }
        WorkflowCommand::Cancel(args) => {
            let cancelled = client::cancel_workflow(host, &args.workflow_id, &args.reason).await?;
            println!("cancelled={cancelled}");
        }
    }
    Ok(())
}
