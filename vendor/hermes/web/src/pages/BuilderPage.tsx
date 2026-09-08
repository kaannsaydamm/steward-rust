import { useCallback, useEffect, useState } from "react";
import { H2 } from "@nous-research/ui/ui/components/typography/h2";
import { Badge } from "@nous-research/ui/ui/components/badge";
import { Button } from "@nous-research/ui/ui/components/button";
import { Card, CardContent } from "@nous-research/ui/ui/components/card";
import { Input } from "@nous-research/ui/ui/components/input";
import { Label } from "@nous-research/ui/ui/components/label";
import { useToast } from "@nous-research/ui/hooks/use-toast";
import { Toast } from "@nous-research/ui/ui/components/toast";
import { api } from "@/lib/api";
import { cn } from "@/lib/utils";

/**
 * Steward Agent Builder — AutoGen-derived agent profiles (schema_version 1
 * YAML) created/edited against the Steward daemon's authoritative profile
 * store (~/.steward/agents/*.yaml) through the gateway's
 * /api/steward/profiles REST proxy (daemon gRPC behind it). The desktop
 * /builder page edits the same store — one source of truth across surfaces.
 */

interface ProfileRow {
  id: string;
  display_name: string;
  template: boolean;
  model_policy: string;
  yaml: string;
}

// ModelPolicy serde values (crates/harness/src/agent_profile.rs ModelPolicy).
const MODEL_CHOICES = ["auto", "fast_reasoning", "best", "local"] as const;

const TEMPLATE_YAML = (id: string, title: string, model: string) => `schema_version: 1
id: ${id}
title: ${title}
description: What this agent is for.
model: ${model}
context:
  mode: fresh
  inherit: [task]
  memory:
    project: read
    user: none
tools:
  discovery: true
  allow_effects: [filesystem_read, git_read]
workspace:
  mode: read_only
budget:
  max_model_calls: 16
  max_tool_calls: 64
spawn: inline
persona: {}
hooks: []
skills: []
subagents: []
template: false
`;

const ID_RE = /^[a-z0-9][a-z0-9_-]{0,63}$/;

function deriveId(title: string): string {
  return title
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+/, "")
    .slice(0, 64);
}

export default function BuilderPage() {
  const { toast, showToast } = useToast();
  const [profiles, setProfiles] = useState<ProfileRow[] | null>(null);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [model, setModel] = useState<string>("auto");
  const [yaml, setYaml] = useState(TEMPLATE_YAML("my-agent", "My Agent", "auto"));
  const [yamlEdited, setYamlEdited] = useState(false);
  const [saving, setSaving] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const res = await api.getStewardProfiles();
      setProfiles(res.profiles ?? []);
    } catch (err) {
      setProfiles([]);
      showToast(`Could not load profiles: ${err}`, "error");
    }
  }, [showToast]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Form fields regenerate the YAML unless the user hand-edited it.
  const derivedId = deriveId(title);
  const effectiveYaml = yamlEdited
    ? yaml
    : TEMPLATE_YAML(derivedId || "my-agent", title.trim() || "My Agent", model);

  const loadOne = (row: ProfileRow) => {
    setSelectedId(row.id);
    setTitle(row.display_name && row.display_name !== row.id ? row.display_name : row.id);
    setDescription("");
    setYaml(row.yaml || TEMPLATE_YAML(row.id, row.display_name || row.id, "auto"));
    setYamlEdited(true);
  };

  const resetForm = () => {
    setSelectedId(null);
    setTitle("");
    setDescription("");
    setModel("auto");
    setYaml(TEMPLATE_YAML("my-agent", "My Agent", "auto"));
    setYamlEdited(false);
  };

  const save = async () => {
    if (!derivedId) return;
    setSaving(true);
    try {
      await api.saveStewardProfile({ yaml: effectiveYaml });
      showToast(`Profile saved: ${derivedId}`, "success");
      await refresh();
      resetForm();
    } catch (err) {
      showToast(`Save failed: ${err}`, "error");
    } finally {
      setSaving(false);
    }
  };

  const remove = async (id: string) => {
    try {
      await api.deleteStewardProfile(id);
      showToast(`Profile deleted: ${id}`, "success");
      if (selectedId === id) resetForm();
      await refresh();
    } catch (err) {
      showToast(`Delete failed: ${err}`, "error");
    }
  };

  const idValid = derivedId.length > 0 && ID_RE.test(derivedId);

  return (
    <div className="mx-auto w-full max-w-5xl space-y-6 p-4">
      <div className="flex items-center justify-between">
        <H2>Agent Builder</H2>
        <Button ghost onClick={resetForm}>
          New profile
        </Button>
      </div>

      <div className="flex flex-col gap-6 lg:flex-row">
        {/* Left: existing profiles */}
        <Card className="lg:w-64 lg:shrink-0">
          <CardContent className="space-y-2 p-4">
            <div className="text-xs uppercase tracking-wider text-muted-foreground">
              Profiles
            </div>
            {profiles === null && (
              <p className="text-sm text-muted-foreground">Loading…</p>
            )}
            {profiles?.length === 0 && (
              <p className="text-sm text-muted-foreground">No profiles yet.</p>
            )}
            <div className="max-h-96 space-y-1 overflow-y-auto">
              {profiles?.map((p) => (
                <div
                  key={p.id}
                  className={cn(
                    "group flex items-center justify-between rounded px-2 py-1.5 text-sm hover:bg-muted",
                    selectedId === p.id && "bg-muted",
                  )}
                >
                  <button
                    type="button"
                    className="min-w-0 flex-1 truncate text-left"
                    onClick={() => loadOne(p)}
                    title={p.id}
                  >
                    <span className="font-medium">{p.display_name || p.id}</span>
                    {p.template && (
                      <Badge tone="outline" className="ml-2">
                        template
                      </Badge>
                    )}
                  </button>
                  <Button
                    size="sm"
                    ghost
                    destructive
                    className="shrink-0 opacity-0 transition-opacity group-hover:opacity-100"
                    onClick={() => void remove(p.id)}
                    aria-label={`Delete ${p.id}`}
                  >
                    Delete
                  </Button>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        {/* Right: editor */}
        <Card className="min-w-0 flex-1">
          <CardContent className="space-y-4 p-5">
            <div className="grid gap-4 md:grid-cols-2">
              <div className="grid gap-1.5">
                <Label htmlFor="ab-title">Title</Label>
                <Input
                  id="ab-title"
                  placeholder="My Agent"
                  value={title}
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                    setTitle(e.target.value);
                    setYamlEdited(false);
                  }}
                />
                <p className="text-xs text-muted-foreground">
                  id: {derivedId || "—"}
                  {derivedId && !idValid && (
                    <span className="text-destructive">
                      {" "}
                      (lowercase letters, digits, hyphens, underscores)
                    </span>
                  )}
                </p>
              </div>
              <div className="grid gap-1.5">
                <Label htmlFor="ab-model">Model policy</Label>
                <select
                  id="ab-model"
                  className="h-9 w-full border border-border bg-background/40 px-3 text-sm shadow-sm focus-visible:border-foreground/25 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-foreground/30"
                  value={model}
                  onChange={(e: React.ChangeEvent<HTMLSelectElement>) => {
                    setModel(e.target.value);
                    setYamlEdited(false);
                  }}
                >
                  {MODEL_CHOICES.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            <div className="grid gap-1.5">
              <Label htmlFor="ab-description">Description</Label>
              <Input
                id="ab-description"
                placeholder="What this agent is for."
                value={description}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                  setDescription(e.target.value);
                  setYamlEdited(false);
                }}
              />
            </div>

            <div className="grid gap-1.5">
              <Label htmlFor="ab-yaml">Profile YAML (schema_version 1)</Label>
              <textarea
                id="ab-yaml"
                className="flex h-80 w-full border border-border bg-background/40 px-3 py-2 text-xs font-courier leading-relaxed shadow-sm placeholder:text-muted-foreground focus-visible:border-foreground/25 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-foreground/30"
                spellCheck={false}
                value={effectiveYaml}
                onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => {
                  setYaml(e.target.value);
                  setYamlEdited(true);
                }}
              />
              {yamlEdited && (
                <p className="text-xs text-muted-foreground">
                  Hand-edited — form fields no longer regenerate the YAML.
                </p>
              )}
            </div>

            <div className="flex items-center justify-between">
              <p className="text-xs text-muted-foreground">
                Saved to the Steward daemon store (~/.steward) — shared by the
                CLI, the desktop app and this dashboard.
              </p>
              <Button onClick={() => void save()} disabled={saving || !idValid}>
                {saving ? "Saving…" : "Save profile"}
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>

      <Toast toast={toast} />
    </div>
  );
}
