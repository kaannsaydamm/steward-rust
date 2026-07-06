"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { stewardClient } from "@/lib/grpc";
import type { ProviderCatalogEntry, ProviderProfileInfo } from "@/lib/proto/steward";

export default function ProvidersTab() {
  const [catalog, setCatalog] = useState<ProviderCatalogEntry[]>([]);
  const [profiles, setProfiles] = useState<ProviderProfileInfo[]>([]);
  const [providerId, setProviderId] = useState("openai");
  const [profileId, setProfileId] = useState("default");
  const [model, setModel] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [keyEnv, setKeyEnv] = useState("");
  const [message, setMessage] = useState("");

  const load = useCallback(async () => {
    const [catalogResponse, profileResponse] = await Promise.all([
      stewardClient.listProviderCatalog({}),
      stewardClient.listProviderProfiles({}),
    ]);
    setCatalog(catalogResponse.providers);
    setProfiles(profileResponse.profiles);
    return catalogResponse.providers;
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void load().then((providers) => applyCatalogDefaults(providers[0]));
    }, 0);
    return () => window.clearTimeout(timer);
  }, [load]);

  function applyCatalogDefaults(provider?: ProviderCatalogEntry) {
    if (!provider) return;
    setProviderId(provider.providerId);
    setBaseUrl(provider.defaultBaseUrl);
    setKeyEnv(provider.defaultApiKeyEnv);
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    const provider = catalog.find((item) => item.providerId === providerId);
    if (!provider) return;
    try {
      await stewardClient.saveProviderProfile({
        activate: true,
        profile: {
          profileId: profileId.trim(),
          providerId,
          displayName: provider.name,
          protocol: provider.protocol,
          baseUrl: baseUrl.trim(),
          model: model.trim(),
          apiKeyEnv: keyEnv.trim(),
          active: false,
        },
      });
      await load();
      setMessage("Profile saved and activated.");
    } catch (reason) {
      setMessage(reason instanceof Error ? reason.message : String(reason));
    }
  }

  return (
    <div className="flex-1 overflow-y-auto p-4 md:p-8">
      <div className="mx-auto max-w-6xl">
        <header className="mb-8">
          <p className="font-mono text-[10px] uppercase tracking-[0.25em] text-primary">Model gateway</p>
          <h2 className="mt-2 font-serif text-3xl">Providers</h2>
          <p className="mt-2 max-w-2xl text-sm text-outline">Profiles keep endpoint and model settings in ~/.steward. API secrets stay in the named environment variable and are never written to disk.</p>
        </header>
        <div className="grid gap-6 xl:grid-cols-[1fr_1.2fr]">
          <section className="card-ghost p-5">
            <h3 className="mb-4 font-mono text-xs uppercase tracking-widest text-primary">Configured</h3>
            <div className="space-y-2">
              {profiles.length === 0 && <p className="text-sm text-outline">No provider profile configured.</p>}
              {profiles.map((profile) => (
                <div key={profile.profileId} className="flex items-center justify-between gap-2 border border-outline-variant/30 p-3">
                  <div className="min-w-0">
                    <p className="truncate text-sm">{profile.displayName} / {profile.model}</p>
                    <p className="truncate font-mono text-[10px] text-outline">{profile.profileId} · {profile.apiKeyEnv || "no key"}</p>
                  </div>
                  <div className="flex shrink-0 items-center gap-2">
                    {profile.active ? (
                      <span className="chip-bracket text-primary">Active</span>
                    ) : (
                      <button className="btn-ghost" onClick={async () => { await stewardClient.activateProviderProfile({ profileId: profile.profileId }); await load(); }}>Use</button>
                    )}
                    <button
                      className="border border-error/40 px-3 py-1 font-mono text-[10px] uppercase text-error"
                      onClick={async () => {
                        await stewardClient.deleteProviderProfile({ profileId: profile.profileId });
                        await load();
                      }}
                    >
                      Remove
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </section>
          <form onSubmit={save} className="card-ghost space-y-4 p-5">
            <h3 className="font-mono text-xs uppercase tracking-widest text-primary">Add or update profile</h3>
            <label className="block text-xs text-outline">Provider
              <select
                value={providerId}
                onChange={(event) => applyCatalogDefaults(catalog.find((item) => item.providerId === event.target.value))}
                className="mt-1 w-full border border-outline-variant/50 bg-surface-container p-3 text-sm"
              >
                {catalog.map((provider) => <option key={provider.providerId} value={provider.providerId}>{provider.name}</option>)}
              </select>
            </label>
            <div className="grid gap-4 md:grid-cols-2">
              <Field label="Profile ID" value={profileId} onChange={setProfileId} />
              <Field label="Model ID" value={model} onChange={setModel} placeholder="e.g. gpt-5.4" />
            </div>
            <Field label="Base URL" value={baseUrl} onChange={setBaseUrl} />
            <Field label="API key environment variable" value={keyEnv} onChange={setKeyEnv} placeholder="OPENAI_API_KEY" />
            <button className="btn-ghost" disabled={!profileId.trim() || !model.trim() || !baseUrl.trim()}>Save and activate</button>
            {message && <p role="status" className="text-sm text-primary">{message}</p>}
          </form>
        </div>
      </div>
    </div>
  );
}

function Field({ label, value, onChange, placeholder = "" }: { label: string; value: string; onChange: (value: string) => void; placeholder?: string }) {
  return (
    <label className="block text-xs text-outline">{label}
      <input value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} className="input-ledger mt-1 w-full" />
    </label>
  );
}
