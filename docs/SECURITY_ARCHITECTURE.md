# Security Architecture (Omega Phase 26)

Normative invariants from the master plan (§13.7) mapped to their enforcing
component and regression test. Every row is enforced by code, not prose.

## Trust boundaries

| # | Invariant | Enforced by | Test |
|---|---|---|---|
| 1 | Untrusted web/MCP/tool text cannot write system policy | `context::TrustLevel` (UntrustedExternal can never be a policy source) | `steward-context` trust labels |
| 2 | Tool name is not permission; effects are | `tools::EffectPolicy` deny-wins evaluation | `alias_cannot_bypass_effects` |
| 3 | RLM sidecars share no daemon capability | `code_runtime` bridge — every call policy-checked; sidecar env is cleared | `secret_env_never_inherited_by_default` |
| 4 | Hooks/extensions are not policy bypasses | `hooks::register` refuses shell/webhook without effect grants | `shell_hooks_require_process_effect` |
| 5 | API keys never in plaintext config/export/logs | `secrets::OsSecretStore` + `ProviderSettings::save` strip | `legacy_inline_key_is_vaulted_on_save_and_stripped_from_disk`, live `saved_key_never_lands_in_providers_json` |
| 6 | Provider keys never to plain-HTTP remote | `ProviderProfile::validate` + transport guard (issue #4, S-014) | live evals |
| 7 | Path canonicalization blocks `..`, symlink escape | `workspace::WorkspaceLease::resolve`, `coding` safe paths | `dotdot_traversal_is_rejected`, `symlink_escape_is_rejected` |
| 8 | Worktree cleanup never deletes unrelated paths | `worktree::remove` managed-root check | `cleanup_refuses_paths_outside_managed_root` |
| 9 | Shell env is secret-minimized by default | `process::spawn` env_clear + allowlist | S-005 |
| 10 | Loopback origin is not the only auth layer | `app_server::origin_allowed` + wire auth token before commands | `upgrade_rejects_foreign_origins`, `hello_rejects_wrong_token` |
| 11 | No secret-bearing GET query params | wire envelope carries auth in Hello frame, not URL | wire tests |
| 12 | Approval decisions are scoped and auditable | `tools::ApprovalStore` scope/expiration/provenance | `once_scope_binds_to_exact_args_and_run`, `provenance_is_recorded` |
| 13 | Self-refine cannot change protected policy files without gates | `refine::activate` manual gate + eval gates + Git commit | `manual_policy_refuses_auto_activation`, `failing_evals_reject_candidate` |
| 14 | Self-improvement evals never use production credentials | refine pipeline operates on `ContextRepo` files only | refine tests |
| 15 | Nightly dream cannot silently alter harness behavior | `Activation::Manual` default | same |
| 16 | Imported skills cannot auto-activate privileged | skills enter as metadata; effects validated on load | `index_records_metadata_without_loading_bodies` |
| 17 | Skill package provenance (hash) recorded at install | `SkillProvenance.content_hash` | skills tests |
| 18 | Runtime downloads verified before use | runtime manager specs (version-pinned, checksummed) | `RUNTIME_SPECS` |
| 19 | Untrusted content in artifact viewer not executed | `ResourceReader` renders metadata only for binaries | `binaries_report_metadata_only` |
| 20 | MCP server description cannot raise own effect scope | MCP tools normalize to ToolSpec; policy reads declared effects | `mcp_and_builtin_unify_on_one_shape` |
| 21 | DAP sessions policy-gated (execute user code) | `DapManager::launch(effect_granted)` | `launch_requires_policy_grant` |
| 22 | Sidecar cell timeout / cancel | `execute_with_timeout`, session cancel | `cell_timeout_returns_error_response` |

## Dependency-selection policy (§13.11)

1. Abstraction boundary defined by Steward traits first.
2. Dependency appears only in the implementing adapter.
3. License, maintenance, cross-platform, binary size, security reviewed.
4. Vendor-specific runtime never leaks into core schema.
5. Swappable library never forces a kernel redesign.

## Residual risks

- Wire v2 auth token rotation is per-daemon-start (persistent token lands
  with the Desktop phase); loopback + origin check remain active layers.
- OCI/SSH sandbox backends delegate isolation to the container/SSH host;
  their conformance suite runs only where the backend exists.
- `cargo fuzz` targets land with the Wire binary codec; JSON-diagnostic
  fuzzing is covered by envelope roundtrip property tests today.
