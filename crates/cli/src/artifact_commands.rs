use crate::cli::{ArtifactCommand, ArtifactKindArg};
use crate::client;
use anyhow::{Context as _, Result};
use std::io::Read as _;
use steward_core::pb::ArtifactKind;

pub async fn run(host: &str, command: ArtifactCommand) -> Result<()> {
    match command {
        ArtifactCommand::List(args) => {
            let artifacts = client::list_artifacts(host, &args.query).await?;
            if artifacts.is_empty() {
                println!("no artifacts");
            }
            for artifact in artifacts {
                println!(
                    "{}\t{}\t{}\t{} bytes",
                    artifact.artifact_id,
                    kind_label(artifact.kind),
                    artifact.title,
                    artifact.content.len()
                );
            }
        }
        ArtifactCommand::Create(args) => {
            let content = match args.content {
                Some(content) => content,
                None => {
                    let mut buffer = String::new();
                    std::io::stdin()
                        .read_to_string(&mut buffer)
                        .context("reading artifact content from stdin")?;
                    buffer
                }
            };
            let artifact = client::create_artifact(
                host,
                &args.title,
                kind_code(args.kind),
                &content,
                &args.language,
                &args.session_id,
            )
            .await?;
            println!("created\t{}\t{}", artifact.artifact_id, artifact.title);
        }
        ArtifactCommand::Show(args) => {
            let artifact = client::get_artifact(host, &args.artifact_id).await?;
            println!("title: {}", artifact.title);
            println!("kind: {}", kind_label(artifact.kind));
            if !artifact.language.is_empty() {
                println!("language: {}", artifact.language);
            }
            println!();
            println!("{}", artifact.content);
        }
        ArtifactCommand::Delete(args) => {
            let deleted = client::delete_artifact(host, &args.artifact_id).await?;
            println!(
                "{}\t{}",
                if deleted { "deleted" } else { "not found" },
                args.artifact_id
            );
        }
    }
    Ok(())
}

fn kind_code(kind: ArtifactKindArg) -> i32 {
    match kind {
        ArtifactKindArg::Code => ArtifactKind::Code as i32,
        ArtifactKindArg::Text => ArtifactKind::Text as i32,
        ArtifactKindArg::Markdown => ArtifactKind::Markdown as i32,
        ArtifactKindArg::Image => ArtifactKind::Image as i32,
        ArtifactKindArg::Link => ArtifactKind::Link as i32,
        ArtifactKindArg::Diff => ArtifactKind::Diff as i32,
    }
}

fn kind_label(code: i32) -> &'static str {
    match ArtifactKind::try_from(code) {
        Ok(ArtifactKind::Code) => "code",
        Ok(ArtifactKind::Text) => "text",
        Ok(ArtifactKind::Markdown) => "markdown",
        Ok(ArtifactKind::Image) => "image",
        Ok(ArtifactKind::Link) => "link",
        Ok(ArtifactKind::Diff) => "diff",
        _ => "unknown",
    }
}
