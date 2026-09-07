"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { stewardClient } from "@/lib/grpc";
import type { ProviderCatalogEntry, ProviderProfileInfo } from "@/lib/proto/steward";
import { StatusPill } from "./StatusPill";
import { useTranslation } from "@/lib/i18n/context";

export default function ProvidersTab() {
  const { t } = useTranslation();
  const [catalog, setCatalog] = useState<ProviderCatalogEntry[]>([]);
  const [profiles, setProfiles] = useState<ProviderProfileInfo[]>([]);
  const [providerId, setProviderId] = useState("openai");
  const [profileId, setProfileId] = useState("default");
  const [model, setModel] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [keyEnv, setKeyEnv] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [message, setMessage] = useState("");
  const [discoveredModels, setDiscoveredModels] = useState<string[]>([]);
  const [discoverError, setDiscoverError] = useState("");
  const [discovering, setDiscovering] = useState(false);

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
    setDiscoveredModels([]);
    setDiscoverError("");
  }

  async function discoverModels() {
    const provider = catalog.find((item) => item.providerId === providerId);
    if (!provider) return;
    setDiscovering(true);
    setDiscoverError("");
    try {
      const response = await stewardClient.listProviderModels({
        protocol: provider.protocol,
        baseUrl: baseUrl.trim(),
        apiKeyEnv: keyEnv.trim(),
        apiKey: apiKey.trim(),
      });
      if (response.error) {
        setDiscoverError(response.error);
        setDiscoveredModels([]);
      } else {
        setDiscoveredModels(response.modelIds);
        if (response.modelIds.length === 0) setDiscoverError("Provider returned no models.");
      }
    } catch (caught) {
      setDiscoverError(caught instanceof Error ? caught.message : "Model discovery failed");
    } finally {
      setDiscovering(false);
    }
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
          apiKey: apiKey.trim(),
          hasStoredKey: false,
        },
      });
      await load();
      setApiKey("");
      setMessage("Profile saved and activated.");
    } catch (reason) {
      setMessage(reason instanceof Error ? reason.message : String(reason));
    }
  }

  return (
    <div className="flex-1 overflow-y-auto p-4 md:p-8">
      <div className="mx-auto max-w-6xl">
        <header className="mb-8">
          <p className="font-mono text-[10px] uppercase tracking-[0.25em] text-primary">{t("providers.eyebrow")}</p>
          <h2 className="mt-2 font-serif text-3xl">{t("nav.providers")}</h2>
          <p className="mt-2 max-w-2xl text-sm text-outline">{t("providers.subtitle")}</p>
        </header>
        <section className="mb-8">
          <h3 className="mb-4 font-mono text-xs uppercase tracking-widest text-primary">
            Configured / {profiles.length}
          </h3>
          {profiles.length === 0 ? (
            <p className="text-sm text-outline">No provider profile configured.</p>
          ) : (
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
              {profiles.map((profile) => (
                <div key={profile.profileId} className="card-ghost flex flex-col gap-3 p-5">
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <p className="truncate text-sm text-on-surface">{profile.displayName}</p>
                      <p className="truncate font-mono text-[10px] text-outline">{profile.model}</p>
                    </div>
                    <StatusPill tone={profile.active ? "success" : "neutral"}>
                      {profile.active ? "Active" : "Idle"}
                    </StatusPill>
                  </div>
                  <p className="truncate font-mono text-[10px] text-outline/70">
                    {profile.profileId} ·{" "}
                    {profile.hasStoredKey
                      ? "stored key"
                      : profile.apiKeyEnv || "no key"}
                  </p>
                  <div className="mt-auto flex gap-2 pt-2">
                    {!profile.active && (
                      <button
                        className="border border-outline/30 px-3 py-1 font-label-mono text-[9px] uppercase text-primary"
                        onClick={async () => { await stewardClient.activateProviderProfile({ profileId: profile.profileId }); await load(); }}
                      >
                        Use
                      </button>
                    )}
                    <button
                      className="border border-error/30 px-3 py-1 font-label-mono text-[9px] uppercase text-error"
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
          )}
        </section>

        <div className="grid gap-6 xl:grid-cols-[1fr_1.2fr]">
          <section className="card-ghost p-5">
            <h3 className="mb-4 font-mono text-xs uppercase tracking-widest text-primary">Available providers</h3>
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
              {catalog.map((provider) => (
                <button
                  key={provider.providerId}
                  type="button"
                  onClick={() => applyCatalogDefaults(provider)}
                  className={`border p-3 text-left text-sm transition-colors ${
                    providerId === provider.providerId
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-outline-variant/30 text-on-surface hover:border-primary/40"
                  }`}
                >
                  <p className="truncate">{provider.name}</p>
                  <p className="truncate font-mono text-[10px] text-outline">{provider.providerId}</p>
                </button>
              ))}
            </div>
          </section>
          <form onSubmit={save} className="card-ghost space-y-4 p-5">
            <h3 className="font-mono text-xs uppercase tracking-widest text-primary">Add or update profile</h3>
            <p className="font-mono text-[10px] uppercase tracking-widest text-outline">
              Provider: <span className="text-primary">{catalog.find((item) => item.providerId === providerId)?.name ?? providerId}</span>
              {" "}(pick one from the grid on the left)
            </p>
            <div className="grid gap-4 md:grid-cols-2">
              <Field label="Profile ID" value={profileId} onChange={setProfileId} />
              <Field label="Model ID" value={model} onChange={setModel} placeholder="e.g. gpt-5.4" />
            </div>
            <Field label="Base URL" value={baseUrl} onChange={setBaseUrl} />
            <div className="grid gap-4 md:grid-cols-2">
              <Field label="API key environment variable" value={keyEnv} onChange={setKeyEnv} placeholder="OPENAI_API_KEY" />
              <Field
                label="API key (stored in OS vault)"
                value={apiKey}
                onChange={setApiKey}
                placeholder="leave blank to keep existing / use env var"
                type="password"
              />
            </div>
            <div>
              <button
                type="button"
                onClick={() => void discoverModels()}
                disabled={discovering || !baseUrl.trim()}
                className="border border-outline/30 px-3 py-1.5 font-label-mono text-[10px] uppercase tracking-widest text-primary disabled:opacity-40"
              >
                {discovering ? "Querying..." : "Discover models"}
              </button>
              {discoverError && <p className="mt-2 text-xs text-error">{discoverError}</p>}
              {discoveredModels.length > 0 && (
                <div className="mt-2 max-h-40 overflow-y-auto border border-outline-variant/30">
                  {discoveredModels.map((modelId) => (
                    <button
                      key={modelId}
                      type="button"
                      onClick={() => setModel(modelId)}
                      className={`block w-full truncate px-3 py-1.5 text-left font-mono text-xs ${
                        model === modelId ? "bg-primary/10 text-primary" : "text-on-surface hover:bg-surface-container"
                      }`}
                    >
                      {modelId}
                    </button>
                  ))}
                </div>
              )}
            </div>
            <button className="btn-ghost" disabled={!profileId.trim() || !model.trim() || !baseUrl.trim()}>Save and activate</button>
            {message && <p role="status" className="text-sm text-primary">{message}</p>}
          </form>
        </div>
      </div>
    </div>
  );
}

function Field({
  label,
  value,
  onChange,
  placeholder = "",
  type = "text",
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  type?: string;
}) {
  return (
    <label className="block text-xs text-outline">{label}
      <input
        type={type}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        className="input-ledger mt-1 w-full"
      />
    </label>
  );
}
