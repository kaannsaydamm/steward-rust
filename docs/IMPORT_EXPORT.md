# Import / Export v2 (Omega Phase 27)

## Omega archive manifest

```text
manifest.json      # { "archive_version": 1, "source": "steward-<ver>", "created_at_ms": ... }
config/            # providers.json (secret refs only), security.json
database/          # steward.db (schema migrated to current)
context/           # Git-backed context repo bundle
harness/           # Git-backed harness repo snapshot
workflows/         # ExecutionPlan JSON files
```

**Excluded always:** plaintext secrets, OS keychain private material, live
process handles, stale auth tokens (§12.15).

## Import flow

1. **Dry-run** (Task 27.3): parse manifest, report source version, entity
   counts, conflicts, skipped secrets, required migrations — without writing.
2. **Atomic staged import**: staged temp dirs, migrate DB, swap on success.
   Failure leaves the current install untouched.
3. **Secret re-bind**: vault references are listed as
   `reconnect N credentials`; values are never in the archive.

## Dry-run report schema

```json
{
  "archive_version": 1,
  "source": "steward-0.1.0",
  "entities": {"threads": 12, "runs": 340, "workflows": 4, "skills": 2},
  "conflicts": [],
  "skipped_secrets": ["steward://secret/provider/live-test"],
  "required_migrations": [2]
}
```

## Portability tests (Task 27.4)

Windows/Unix path fixtures; archive traversal defense (`..`, absolute,
symlink) — enforced by the same `safe_path` discipline as the context repo.
