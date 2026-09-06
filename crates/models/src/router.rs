//! Task-aware router (§21.2, Task 6.3, M-007): hard capability constraints
//! always beat soft scores; local-only privacy cannot route to remote.

use crate::catalog::{Catalog, RouteId};
use crate::health::Health;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Chat,
    CodingHeavy,
    FastLookup,
    LongContext,
    Vision,
}

/// Hard constraints: any violation disqualifies a route entirely (M-007).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Requirements {
    pub tool_use: bool,
    pub vision: bool,
    pub structured_output: bool,
    pub reasoning_controls: bool,
    pub min_context_tokens: u64,
    /// Privacy: only local endpoints may serve the request.
    pub local_only: bool,
    /// Routes the user explicitly excluded.
    #[serde(default)]
    pub excluded: Vec<String>,
    /// User pin: if set and healthy, it wins outright.
    #[serde(default)]
    pub pinned: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RouteRequest {
    pub task: TaskKind,
    pub requirements: Requirements,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectedRoute {
    pub route: RouteId,
    /// Why the router picked this (persisted as route rationale, M-007).
    pub rationale: String,
}

pub struct Router {
    pub health: Health,
}

impl Router {
    pub fn new(health: Health) -> Self {
        Self { health }
    }

    /// Selects the best available route or fails with an actionable reason.
    pub fn select(
        &self,
        catalog: &Catalog,
        request: &RouteRequest,
        now: Instant,
    ) -> Result<SelectedRoute> {
        // User pin short-circuits everything when healthy.
        if let Some(pinned) = &request.requirements.pinned {
            if self.health.is_available(pinned, now) {
                return Ok(SelectedRoute {
                    route: RouteId(pinned.clone()),
                    rationale: "user_pinned".into(),
                });
            }
            bail!("pinned model '{pinned}' is unavailable; unpin it or wait for recovery");
        }

        let mut viable: Vec<(f32, String, String)> = Vec::new();
        for entry in catalog.all() {
            let route = &entry.route.0;
            if request.requirements.excluded.iter().any(|x| x == route) {
                continue;
            }
            let caps = &entry.capabilities;
            // Hard gates (in order of user impact).
            if request.requirements.tool_use && !caps.tool_use {
                continue;
            }
            if request.requirements.vision && !caps.vision {
                continue;
            }
            if request.requirements.structured_output && !caps.structured_output {
                continue;
            }
            if request.requirements.reasoning_controls && !caps.reasoning_controls {
                continue;
            }
            if caps.max_context_tokens < request.requirements.min_context_tokens {
                continue;
            }
            if request.requirements.local_only && !caps.local_only {
                continue;
            }
            if !self.health.is_available(route, now) {
                continue;
            }

            // Soft score: task affinity + cost + latency.
            let mut score = 0.0_f32;
            score -= caps.cost_class as f32 / 2550.0;
            score -= caps.latency_class as f32 / 2550.0;
            score += match request.task {
                TaskKind::CodingHeavy if caps.tool_use && caps.max_context_tokens >= 128_000 => 0.5,
                TaskKind::FastLookup if caps.latency_class <= 64 => 0.5,
                TaskKind::LongContext if caps.max_context_tokens >= 200_000 => 0.5,
                TaskKind::Vision if caps.vision => 0.5,
                TaskKind::Chat => 0.25,
                _ => 0.0,
            };
            viable.push((score, route.clone(), String::new()));
        }

        if viable.is_empty() {
            bail!(
                "no model route satisfies the task requirements (task={:?}); \
                 register a capable model or relax constraints",
                request.task
            );
        }
        viable.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.cmp(&b.1))
        });
        let (score, route, _) = viable.remove(0);
        Ok(SelectedRoute {
            route: RouteId(route),
            rationale: format!("task_affinity score={score:.3}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::ModelCapabilities;

    fn entry(route: &str, caps: ModelCapabilities) -> crate::catalog::ModelEntry {
        crate::catalog::ModelEntry {
            route: RouteId(route.into()),
            capabilities: caps,
            tags: vec![],
        }
    }

    fn tools(caps: bool, cost: u8, latency: u8, ctx: u64, local: bool) -> ModelCapabilities {
        ModelCapabilities {
            tool_use: caps,
            cost_class: cost,
            latency_class: latency,
            max_context_tokens: ctx,
            local_only: local,
            ..Default::default()
        }
    }

    fn request(task: TaskKind, req: Requirements) -> RouteRequest {
        RouteRequest {
            task,
            requirements: req,
        }
    }

    #[test]
    fn capability_constraints_beat_soft_score() {
        // Cheap remote model with tools vs expensive remote model with tools:
        // cheaper wins (soft). A tool-less model never wins regardless of cost.
        let mut catalog = Catalog::default();
        catalog.register(entry(
            "a/toolful-expensive",
            tools(true, 200, 0, 128_000, false),
        ));
        catalog.register(entry(
            "b/toolless-cheap",
            tools(false, 0, 0, 128_000, false),
        ));

        let router = Router::new(Health::new());
        let req = request(
            TaskKind::CodingHeavy,
            Requirements {
                tool_use: true,
                ..Default::default()
            },
        );
        let selected = router.select(&catalog, &req, Instant::now()).unwrap();
        assert_eq!(selected.route.0, "a/toolful-expensive");
        assert!(selected.rationale.contains("task_affinity"));
    }

    #[test]
    fn local_only_never_routes_to_remote() {
        let mut catalog = Catalog::default();
        catalog.register(entry("remote/best", tools(true, 0, 0, 200_000, false)));
        catalog.register(entry("local/worse", tools(true, 250, 250, 32_000, true)));

        let router = Router::new(Health::new());
        let req = request(
            TaskKind::Chat,
            Requirements {
                tool_use: true,
                local_only: true,
                ..Default::default()
            },
        );
        let selected = router.select(&catalog, &req, Instant::now()).unwrap();
        assert_eq!(selected.route.0, "local/worse");
    }

    #[test]
    fn unsatisfiable_requirements_are_actionable() {
        let mut catalog = Catalog::default();
        catalog.register(entry("a/no-tools", tools(false, 0, 0, 128_000, false)));
        let router = Router::new(Health::new());
        let req = request(
            TaskKind::CodingHeavy,
            Requirements {
                tool_use: true,
                ..Default::default()
            },
        );
        let error = router.select(&catalog, &req, Instant::now()).unwrap_err();
        assert!(error.to_string().contains("no model route satisfies"));
    }

    #[test]
    fn excluded_routes_are_skipped() {
        let mut catalog = Catalog::default();
        catalog.register(entry("a/first", tools(true, 0, 0, 128_000, false)));
        catalog.register(entry("b/second", tools(true, 100, 0, 128_000, false)));
        let router = Router::new(Health::new());
        let req = request(
            TaskKind::Chat,
            Requirements {
                tool_use: true,
                excluded: vec!["a/first".into()],
                ..Default::default()
            },
        );
        let selected = router.select(&catalog, &req, Instant::now()).unwrap();
        assert_eq!(selected.route.0, "b/second");
    }

    #[test]
    fn pinned_model_wins_when_healthy() {
        let mut catalog = Catalog::default();
        catalog.register(entry("a/best", tools(true, 0, 0, 128_000, false)));
        catalog.register(entry("b/pinned", tools(true, 200, 200, 32_000, false)));
        let router = Router::new(Health::new());
        let req = request(
            TaskKind::Chat,
            Requirements {
                tool_use: true,
                pinned: Some("b/pinned".into()),
                ..Default::default()
            },
        );
        let selected = router.select(&catalog, &req, Instant::now()).unwrap();
        assert_eq!(selected.route.0, "b/pinned");
        assert_eq!(selected.rationale, "user_pinned");
    }

    #[test]
    fn unhealthy_routes_are_skipped() {
        let mut catalog = Catalog::default();
        catalog.register(entry("a/down", tools(true, 0, 0, 128_000, false)));
        catalog.register(entry("b/up", tools(true, 100, 0, 128_000, false)));
        let health = Health::new();
        health.observe_unavailable("a/down");
        let router = Router::new(health);
        let req = request(
            TaskKind::Chat,
            Requirements {
                tool_use: true,
                ..Default::default()
            },
        );
        let selected = router.select(&catalog, &req, Instant::now()).unwrap();
        assert_eq!(selected.route.0, "b/up");
    }

    #[test]
    fn long_context_task_prefers_big_windows() {
        let mut catalog = Catalog::default();
        catalog.register(entry("a/small", tools(true, 0, 0, 32_000, false)));
        catalog.register(entry("b/big", tools(true, 100, 100, 400_000, false)));
        let router = Router::new(Health::new());
        let req = request(
            TaskKind::LongContext,
            Requirements {
                tool_use: true,
                ..Default::default()
            },
        );
        let selected = router.select(&catalog, &req, Instant::now()).unwrap();
        assert_eq!(selected.route.0, "b/big");
    }
}
