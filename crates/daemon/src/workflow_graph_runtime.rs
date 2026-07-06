use super::{EventSender, WorkflowRuntime};
use crate::{agent_runtime, workflow_definition, workflow_logs};
use anyhow::{anyhow, Context as _, Result};
use std::collections::HashMap;
use steward_core::pb::{
    ChatEventKind, ChatRequest, WorkflowDefinition, WorkflowEvent, WorkflowNode,
};

impl WorkflowRuntime {
    pub(super) async fn run_definition(
        &self,
        workflow_id: &str,
        title: &str,
        description: &str,
        definition_id: &str,
        sender: Option<&EventSender>,
    ) -> Result<()> {
        let (definition, mut outputs) = {
            let db = self
                .db
                .lock()
                .map_err(|_| anyhow!("Database lock failed"))?;
            let definition = workflow_definition::get(&db, definition_id)?
                .with_context(|| format!("workflow definition '{definition_id}' disappeared"))?;
            let outputs = workflow_definition::load_results(&db, workflow_id)?;
            (definition, outputs)
        };
        let order = workflow_definition::validate_and_order(&definition)?;
        let nodes = definition
            .nodes
            .iter()
            .map(|node| (node.node_id.as_str(), node))
            .collect::<HashMap<_, _>>();

        for node_id in &order {
            if outputs.contains_key(node_id) {
                continue;
            }
            if self.is_cancelled(workflow_id).await {
                self.emit_cancelled(workflow_id, sender).await;
                return Ok(());
            }
            let node = nodes[node_id.as_str()];
            self.emit_node_event(
                workflow_id,
                node,
                "running",
                "Node started",
                progress(&outputs, &order),
                sender,
            )
            .await;
            let prompt = node_prompt(title, description, node, &definition, &outputs);
            let output = self
                .run_node_agent(node, prompt, workflow_id, sender)
                .await?;
            {
                let db = self
                    .db
                    .lock()
                    .map_err(|_| anyhow!("Database lock failed"))?;
                workflow_definition::save_result(&db, workflow_id, node_id, &output)?;
            }
            outputs.insert(node_id.clone(), output.clone());
            workflow_logs::log_agent_phase(
                &self.agent_logs,
                workflow_id,
                title,
                &node.title,
                &node.agent_id,
            )
            .await;
            self.emit_node_event(
                workflow_id,
                node,
                "complete",
                &output,
                progress(&outputs, &order),
                sender,
            )
            .await;
        }
        self.emit_node_event(
            workflow_id,
            &WorkflowNode {
                title: "Completed".to_owned(),
                ..WorkflowNode::default()
            },
            "complete",
            "All workflow nodes completed",
            100.0,
            sender,
        )
        .await;
        Ok(())
    }

    async fn run_node_agent(
        &self,
        node: &WorkflowNode,
        prompt: String,
        workflow_id: &str,
        workflow_sender: Option<&EventSender>,
    ) -> Result<String> {
        let (chat_tx, mut chat_rx) = tokio::sync::mpsc::channel(256);
        let request = ChatRequest {
            message: prompt,
            working_directory: std::env::current_dir()?.display().to_string(),
            allow_tools: node.allow_tools,
            ..ChatRequest::default()
        };
        let run = agent_runtime::run(&self.steward, request, chat_tx);
        tokio::pin!(run);
        loop {
            tokio::select! {
                result = &mut run => return result.map(|result| result.final_text),
                event = chat_rx.recv() => {
                    let Some(Ok(event)) = event else { continue };
                    if matches!(ChatEventKind::try_from(event.kind), Ok(ChatEventKind::ToolStart | ChatEventKind::ToolResult)) {
                        self.emit_node_event(
                            workflow_id,
                            node,
                            "tool",
                            &format!("{}: {}", event.tool_name, event.content),
                            0.0,
                            workflow_sender,
                        ).await;
                    }
                }
            }
        }
    }

    async fn emit_node_event(
        &self,
        workflow_id: &str,
        node: &WorkflowNode,
        status: &str,
        detail: &str,
        progress: f32,
        sender: Option<&EventSender>,
    ) {
        let phase = if node.node_id.is_empty() { 10 } else { 8 };
        let event = WorkflowEvent {
            workflow_id: workflow_id.to_owned(),
            phase,
            agent_id: node.agent_id.clone(),
            message: format!("{}: {status}", node.title),
            detail: detail.chars().take(4_000).collect(),
            progress,
            requires_approval: false,
            approval: None,
        };
        if let Some(sender) = sender {
            let _ = sender.send(Ok(event.clone())).await;
        }
        self.apply_event(workflow_id, &event).await;
        self.persist_workflow_event(&event);
    }

    pub(super) async fn fail_workflow(
        &self,
        workflow_id: &str,
        error: &str,
        sender: Option<&EventSender>,
    ) {
        let event = WorkflowEvent {
            workflow_id: workflow_id.to_owned(),
            phase: 11,
            message: "Workflow failed".to_owned(),
            detail: error.to_owned(),
            ..WorkflowEvent::default()
        };
        if let Some(sender) = sender {
            let _ = sender.send(Ok(event.clone())).await;
        }
        self.apply_event(workflow_id, &event).await;
        self.persist_workflow_event(&event);
    }
}

fn node_prompt(
    title: &str,
    description: &str,
    node: &WorkflowNode,
    definition: &WorkflowDefinition,
    outputs: &HashMap<String, String>,
) -> String {
    let dependencies = definition
        .connections
        .iter()
        .filter(|edge| edge.target_node_id == node.node_id)
        .filter_map(|edge| {
            outputs
                .get(&edge.source_node_id)
                .map(|output| (&edge.source_node_id, output))
        })
        .map(|(id, output)| format!("\n[{id}]\n{output}"))
        .collect::<String>();
    format!(
        "Workflow: {title}\nGoal: {description}\nCurrent node: {}\nInstruction: {}\nDependency outputs:{}",
        node.title,
        node.instruction,
        if dependencies.is_empty() { " none".to_owned() } else { dependencies }
    )
}

fn progress(outputs: &HashMap<String, String>, order: &[String]) -> f32 {
    outputs.len() as f32 / order.len() as f32 * 100.0
}
