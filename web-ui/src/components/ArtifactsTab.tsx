"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { stewardClient, timeAgo } from "@/lib/types";
import type { ArtifactInfo } from "@/lib/types";
import { ArtifactKind } from "@/lib/proto/steward";

const KIND_LABELS: Record<number, string> = {
  [ArtifactKind.ARTIFACT_KIND_CODE]: "Code",
  [ArtifactKind.ARTIFACT_KIND_TEXT]: "Text",
  [ArtifactKind.ARTIFACT_KIND_MARKDOWN]: "Markdown",
  [ArtifactKind.ARTIFACT_KIND_IMAGE]: "Image",
  [ArtifactKind.ARTIFACT_KIND_LINK]: "Link",
  [ArtifactKind.ARTIFACT_KIND_DIFF]: "Diff",
};

const KIND_OPTIONS = [
  { value: ArtifactKind.ARTIFACT_KIND_CODE, label: "Code" },
  { value: ArtifactKind.ARTIFACT_KIND_TEXT, label: "Text" },
  { value: ArtifactKind.ARTIFACT_KIND_MARKDOWN, label: "Markdown" },
  { value: ArtifactKind.ARTIFACT_KIND_IMAGE, label: "Image" },
  { value: ArtifactKind.ARTIFACT_KIND_LINK, label: "Link" },
  { value: ArtifactKind.ARTIFACT_KIND_DIFF, label: "Diff" },
];

export default function ArtifactsTab() {
  const [artifacts, setArtifacts] = useState<ArtifactInfo[]>([]);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<ArtifactInfo | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [creating, setCreating] = useState(false);
  const [title, setTitle] = useState("");
  const [kind, setKind] = useState<ArtifactKind>(ArtifactKind.ARTIFACT_KIND_TEXT);
  const [language, setLanguage] = useState("");
  const [content, setContent] = useState("");

  const load = useCallback(async (searchQuery: string) => {
    try {
      const response = await stewardClient.listArtifacts({ query: searchQuery });
      setArtifacts(response.artifacts);
      setError("");
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Daemon request failed");
    }
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => void load(""), 0);
    return () => window.clearTimeout(timer);
  }, [load]);

  async function search(event: FormEvent) {
    event.preventDefault();
    await load(query);
  }

  async function create(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      await stewardClient.createArtifact({
        title: title.trim(),
        kind,
        content,
        language: language.trim(),
        sessionId: "",
      });
      setTitle("");
      setLanguage("");
      setContent("");
      setCreating(false);
      await load(query);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Daemon request failed");
    } finally {
      setBusy(false);
    }
  }

  async function remove(artifact: ArtifactInfo) {
    setBusy(true);
    try {
      await stewardClient.deleteArtifact({ artifactId: artifact.artifactId });
      if (selected?.artifactId === artifact.artifactId) setSelected(null);
      await load(query);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex-1 overflow-y-auto p-4 md:p-8">
      <div className="mx-auto max-w-6xl">
        <header className="mb-8 flex flex-wrap items-end justify-between gap-4">
          <div>
            <p className="font-mono text-[10px] uppercase tracking-[0.25em] text-primary">Output library</p>
            <h2 className="mt-2 font-serif text-3xl">Artifacts</h2>
            <p className="mt-2 max-w-2xl text-sm text-outline">
              Code, text, links, and diffs saved from sessions — searchable independent of chat history.
            </p>
          </div>
          <button
            type="button"
            onClick={() => setCreating((open) => !open)}
            className="border border-primary/40 px-4 py-2 font-label-mono text-[10px] uppercase tracking-widest text-primary"
          >
            {creating ? "Cancel" : "New artifact"}
          </button>
        </header>

        {error && <div className="mb-5 border border-error/40 p-3 text-sm text-error">{error}</div>}

        {creating && (
          <form onSubmit={create} className="card-ghost mb-8 grid gap-4 p-5 md:grid-cols-2">
            <label className="block text-xs text-outline md:col-span-2">Title
              <input value={title} onChange={(event) => setTitle(event.target.value)} className="input-ledger mt-1 w-full" required />
            </label>
            <label className="block text-xs text-outline">Kind
              <select
                value={kind}
                onChange={(event) => setKind(Number(event.target.value) as ArtifactKind)}
                className="mt-1 w-full border border-outline-variant/50 bg-surface-container p-3 text-sm"
              >
                {KIND_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
            </label>
            <label className="block text-xs text-outline">Language (optional)
              <input value={language} onChange={(event) => setLanguage(event.target.value)} placeholder="e.g. rust, html" className="input-ledger mt-1 w-full" />
            </label>
            <label className="block text-xs text-outline md:col-span-2">Content
              <textarea
                value={content}
                onChange={(event) => setContent(event.target.value)}
                rows={8}
                className="mt-1 w-full resize-none border border-outline-variant/40 bg-surface-container-low p-3 font-mono text-xs leading-5 outline-none focus:border-primary/60"
                required
              />
            </label>
            <button className="btn-ghost md:col-span-2" disabled={busy || !title.trim() || !content.trim()}>
              Save artifact
            </button>
          </form>
        )}

        <form onSubmit={search} className="mb-6 flex gap-2">
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search artifacts..."
            className="input-ledger flex-1"
          />
          <button className="btn-ghost">Search</button>
        </form>

        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {artifacts.map((artifact) => (
            <button
              key={artifact.artifactId}
              type="button"
              onClick={() => setSelected(artifact)}
              className="card-ghost flex flex-col gap-2 p-5 text-left"
            >
              <div className="max-h-24 overflow-hidden font-mono text-[11px] leading-5 text-outline/70">
                {artifact.content.slice(0, 220)}
              </div>
              <p className="truncate text-sm text-on-surface">{artifact.title}</p>
              <div className="flex items-center justify-between font-label-mono text-[9px] uppercase tracking-widest text-primary">
                <span>{KIND_LABELS[artifact.kind] ?? "unknown"}</span>
                <span className="text-outline">{timeAgo(artifact.updatedAt)}</span>
              </div>
            </button>
          ))}
          {artifacts.length === 0 && (
            <p className="col-span-full py-8 text-center font-label-mono text-xs text-outline">
              No artifacts yet
            </p>
          )}
        </div>
      </div>

      {selected && (
        <div
          className="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-4"
          onClick={() => setSelected(null)}
        >
          <div
            className="card-ghost max-h-[80vh] w-full max-w-3xl overflow-y-auto p-6"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="mb-4 flex items-start justify-between gap-4">
              <div>
                <p className="font-mono text-[10px] uppercase tracking-widest text-primary">
                  {KIND_LABELS[selected.kind] ?? "unknown"}{selected.language ? ` · ${selected.language}` : ""}
                </p>
                <h3 className="mt-1 font-serif text-xl">{selected.title}</h3>
              </div>
              <div className="flex shrink-0 gap-2">
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => void remove(selected)}
                  className="border border-error/30 px-3 py-1 font-label-mono text-[9px] uppercase text-error disabled:opacity-40"
                >
                  Delete
                </button>
                <button
                  type="button"
                  onClick={() => setSelected(null)}
                  className="border border-outline/30 px-3 py-1 font-label-mono text-[9px] uppercase text-on-surface"
                >
                  Close
                </button>
              </div>
            </div>
            <pre className="whitespace-pre-wrap break-words font-mono text-xs leading-6 text-on-surface">
              {selected.content}
            </pre>
          </div>
        </div>
      )}
    </div>
  );
}
