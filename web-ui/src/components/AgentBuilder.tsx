// Ported from microsoft/autogen@027ecf0a379bcc1d09956d46d12d44a3ad9cee14
// autogen-studio teambuilder semantics (MIT). Modified for Steward: the form
// maps 1:1 onto AgentProfile fields; YAML round-trips through the daemon's
// SaveAgentProfile/ListAgentProfiles/DeleteAgentProfile RPCs; validation
// errors surface inline from ProfileError.

"use client";

import { FormEvent, useEffect, useState } from "react";
import { stewardClient } from "@/lib/types";
import { useTranslation } from "@/lib/i18n/context";

const MODEL_POLICIES = ["auto", "fast_reasoning", "best", "local"] as const;

interface FormState {
  id: string;
  title: string;
  description: string;
  modelPolicy: string;
  explicitProfileId: string;
  role: string;
  systemInstructions: string;
  hooks: string;
  skills: string;
  subagents: string;
  template: boolean;
}

const EMPTY_FORM: FormState = {
  id: "",
  title: "",
  description: "",
  modelPolicy: "auto",
  explicitProfileId: "",
  role: "",
  systemInstructions: "",
  hooks: "",
  skills: "",
  subagents: "",
  template: false,
};

function formToYaml(form: FormState): string {
  const lines: string[] = [
    "schema_version: 1",
    `id: ${form.id || "unnamed"}`,
    `title: ${form.title || "Untitled"}`,
  ];
  if (form.description) lines.push(`description: ${form.description}`);
  lines.push(
    form.modelPolicy === "explicit" && form.explicitProfileId
      ? `model: !explicit\n  profile_id: ${form.explicitProfileId}`
      : `model: ${form.modelPolicy}`,
    "context:",
    "  mode: fresh",
    "tools:",
    "  discovery: false",
    "  allow_effects: [filesystem_read]",
    "workspace:",
    "  mode: read_write",
  );
  if (form.role || form.systemInstructions) {
    lines.push("persona:");
    lines.push(`  role: ${form.role}`);
    lines.push(`  system_instructions: ${form.systemInstructions}`);
  }
  const csv = (value: string) =>
    value
      .split(",")
      .map((part) => part.trim())
      .filter(Boolean);
  const hooks = csv(form.hooks);
  const skills = csv(form.skills);
  const subagents = csv(form.subagents);
  if (hooks.length > 0) lines.push(`hooks: [${hooks.join(", ")}]`);
  if (skills.length > 0) lines.push(`skills: [${skills.join(", ")}]`);
  if (subagents.length > 0) lines.push(`subagents: [${subagents.join(", ")}]`);
  lines.push(`template: ${form.template}`);
  return lines.join("\n");
}

export default function AgentBuilder() {
  const { t } = useTranslation();
  const [profiles, setProfiles] = useState<{ id: string; error: string }[]>([]);
  const [form, setForm] = useState<FormState>(EMPTY_FORM);
  const [yaml, setYaml] = useState("");
  const [error, setError] = useState("");
  const [saved, setSaved] = useState("");

  async function load() {
    try {
      const response = await stewardClient.listAgentProfiles({});
      setProfiles(response.profiles.map((profile) => ({ id: profile.id, error: profile.error })));
      setError("");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }

  useEffect(() => {
    void load();
  }, []);

  function update<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((current) => {
      const next = { ...current, [key]: value };
      setYaml(formToYaml(next));
      return next;
    });
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    try {
      const source = yaml || formToYaml(form);
      const response = await stewardClient.saveAgentProfile({ yaml: source });
      if (response.error) {
        setError(response.error);
        return;
      }
      setSaved(response.id);
      setError("");
      await load();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }

  async function remove(id: string) {
    try {
      await stewardClient.deleteAgentProfile({ id });
      await load();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }

  return (
    <section className="mx-auto w-full max-w-6xl px-4 py-6 md:px-6">
      <h3 className="font-serif text-lg text-on-surface">{t("builder.title")}</h3>
      <p className="mt-1 mb-4 text-sm text-on-surface-variant/60">{t("builder.subtitle")}</p>
      {error && (
        <p role="alert" className="mb-4 border border-error/40 p-3 text-sm text-error">
          {error}
        </p>
      )}
      <div className="grid gap-6 lg:grid-cols-2">
        <form onSubmit={save} className="space-y-4 border border-outline-variant/20 p-5">
          <div className="grid grid-cols-2 gap-3">
            <label className="block">
              <span className="mb-1 block font-label-mono text-[10px] uppercase tracking-widest text-outline">id</span>
              <input
                value={form.id}
                onChange={(event) => update("id", event.target.value.toLowerCase().replace(/[^a-z0-9-_]/g, "-"))}
                className="w-full border border-outline-variant/40 bg-surface-container-low px-3 py-2 font-mono text-xs text-on-surface outline-none focus:border-primary/60"
              />
            </label>
            <label className="block">
              <span className="mb-1 block font-label-mono text-[10px] uppercase tracking-widest text-outline">title</span>
              <input
                value={form.title}
                onChange={(event) => update("title", event.target.value)}
                className="w-full border border-outline-variant/40 bg-surface-container-low px-3 py-2 text-xs text-on-surface outline-none focus:border-primary/60"
              />
            </label>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <label className="block">
              <span className="mb-1 block font-label-mono text-[10px] uppercase tracking-widest text-outline">model</span>
              <select
                value={form.modelPolicy}
                onChange={(event) => update("modelPolicy", event.target.value)}
                className="w-full border border-outline-variant/40 bg-surface-container-low px-3 py-2 text-xs text-on-surface outline-none"
              >
                {MODEL_POLICIES.map((policy) => (
                  <option key={policy} value={policy}>
                    {policy}
                  </option>
                ))}
                <option value="explicit">explicit</option>
              </select>
            </label>
            {form.modelPolicy === "explicit" && (
              <label className="block">
                <span className="mb-1 block font-label-mono text-[10px] uppercase tracking-widest text-outline">provider profile</span>
                <input
                  value={form.explicitProfileId}
                  onChange={(event) => update("explicitProfileId", event.target.value)}
                  className="w-full border border-outline-variant/40 bg-surface-container-low px-3 py-2 font-mono text-xs text-on-surface outline-none"
                />
              </label>
            )}
          </div>
          <div className="grid grid-cols-2 gap-3">
            <label className="block">
              <span className="mb-1 block font-label-mono text-[10px] uppercase tracking-widest text-outline">role</span>
              <input
                value={form.role}
                onChange={(event) => update("role", event.target.value)}
                className="w-full border border-outline-variant/40 bg-surface-container-low px-3 py-2 text-xs text-on-surface outline-none"
              />
            </label>
            <label className="block">
              <span className="mb-1 block font-label-mono text-[10px] uppercase tracking-widest text-outline">hooks (csv)</span>
              <input
                value={form.hooks}
                onChange={(event) => update("hooks", event.target.value)}
                className="w-full border border-outline-variant/40 bg-surface-container-low px-3 py-2 font-mono text-xs text-on-surface outline-none"
              />
            </label>
          </div>
          <label className="block">
            <span className="mb-1 block font-label-mono text-[10px] uppercase tracking-widest text-outline">system instructions</span>
            <textarea
              rows={3}
              value={form.systemInstructions}
              onChange={(event) => update("systemInstructions", event.target.value)}
              className="w-full resize-y border border-outline-variant/40 bg-surface-container-low px-3 py-2 text-xs text-on-surface outline-none"
            />
          </label>
          <div className="grid grid-cols-2 gap-3">
            <label className="block">
              <span className="mb-1 block font-label-mono text-[10px] uppercase tracking-widest text-outline">skills (csv)</span>
              <input
                value={form.skills}
                onChange={(event) => update("skills", event.target.value)}
                className="w-full border border-outline-variant/40 bg-surface-container-low px-3 py-2 font-mono text-xs text-on-surface outline-none"
              />
            </label>
            <label className="block">
              <span className="mb-1 block font-label-mono text-[10px] uppercase tracking-widest text-outline">subagents (csv)</span>
              <input
                value={form.subagents}
                onChange={(event) => update("subagents", event.target.value)}
                className="w-full border border-outline-variant/40 bg-surface-container-low px-3 py-2 font-mono text-xs text-on-surface outline-none"
              />
            </label>
          </div>
          <label className="flex items-center gap-2 text-xs text-on-surface-variant/70">
            <input
              type="checkbox"
              checked={form.template}
              onChange={(event) => update("template", event.target.checked)}
            />
            template (never executes)
          </label>
          <button type="submit" className="btn-ghost border border-primary/40 px-4 py-2 font-mono text-[11px] uppercase text-primary">
            {t("chat.send")}
          </button>
          {saved && <span className="ml-2 font-mono text-xs text-primary">✓ {saved}</span>}
        </form>
        <div className="space-y-4">
          <div className="border border-outline-variant/20 p-5">
            <p className="mb-2 font-label-mono text-[10px] uppercase tracking-widest text-outline">live yaml</p>
            <pre className="max-h-72 overflow-auto whitespace-pre-wrap font-mono text-[11px] leading-5 text-on-surface-variant/80">
              {yaml || formToYaml(form)}
            </pre>
          </div>
          <div className="border border-outline-variant/20 p-5">
            <p className="mb-2 font-label-mono text-[10px] uppercase tracking-widest text-outline">
              saved profiles ({profiles.length})
            </p>
            {profiles.length === 0 ? (
              <p className="text-xs text-on-surface-variant/50">{t("chat.modelPicker.empty")}</p>
            ) : (
              <ul className="space-y-1">
                {profiles.map((profile) => (
                  <li key={profile.id} className="group flex items-center gap-2 text-xs">
                    <span className="font-mono text-on-surface">{profile.id}</span>
                    {profile.error && <span className="text-error">{profile.error}</span>}
                    <button
                      type="button"
                      onClick={() => void remove(profile.id)}
                      className="ml-auto text-outline opacity-0 group-hover:opacity-100 hover:text-error"
                      aria-label="delete"
                    >
                      ×
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}
