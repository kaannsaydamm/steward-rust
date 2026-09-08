import { useMemo, useState } from 'react'
import { useNavigate } from 'react-router'

import { Codicon } from '@/components/ui/codicon'
import { useI18n } from '@/i18n'
import { Button } from '@/components/ui/button'

import { useGatewayRequest } from '../gateway/hooks/use-gateway-request'
import { Panel, PanelEmpty, PanelHeader } from '../overlays/panel'

/**
 * Steward Agent Builder — AutoGen-derived agent profiles (schema_version 1
 * YAML) created/edited against the Steward daemon through the gateway's
 * steward.profiles.* JSON-RPC proxy.
 */

interface ProfileRow {
  id: string
  display_name: string
  template: boolean
  model_policy: string
  yaml: string
}

const TEMPLATE_YAML = `schema_version: 1
id: my-agent
title: My Agent
description: What this agent is for.
model: auto
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
`

export default function BuilderView() {
  const { t } = useI18n()
  const { requestGateway } = useGatewayRequest()
  const navigate = useNavigate()
  const [profiles, setProfiles] = useState<ProfileRow[]>([])
  const [name, setName] = useState('')
  const [yaml, setYaml] = useState(TEMPLATE_YAML)
  const [status, setStatus] = useState<{ kind: 'ok' | 'error'; text: string } | null>(null)
  const [busy, setBusy] = useState(false)

  const refresh = async () => {
    try {
      const res = await requestGateway<{ profiles: ProfileRow[] }>('steward.profiles.list')
      setProfiles(res.profiles ?? [])
    } catch (err) {
      setStatus({ kind: 'error', text: String(err) })
    }
  }

  useMemo(() => {
    void refresh()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const save = async () => {
    setBusy(true)
    setStatus(null)
    try {
      await requestGateway('steward.profiles.save', { yaml })
      setStatus({ kind: 'ok', text: 'profile saved' })
      await refresh()
    } catch (err) {
      setStatus({ kind: 'error', text: String(err) })
    } finally {
      setBusy(false)
    }
  }

  const remove = async (id: string) => {
    try {
      await requestGateway('steward.profiles.delete', { id })
      await refresh()
    } catch (err) {
      setStatus({ kind: 'error', text: String(err) })
    }
  }

  const loadOne = async (id: string) => {
    const row = profiles.find(p => p.id === id)
    if (row) {
      setName(row.display_name || row.id)
      setYaml(row.yaml || TEMPLATE_YAML)
    }
  }

  const derivedId = name.trim().toLowerCase().replace(/[^a-z0-9_-]+/g, '-').replace(/^-+/, '').slice(0, 64)

  return (
    <Panel onClose={() => navigate('/')}>
      <PanelHeader title="Agent Builder" />
      <div className="flex min-h-0 flex-1 gap-4 p-4">
          {/* Left: existing profiles */}
          <div className="flex w-56 shrink-0 flex-col gap-2 overflow-y-auto">
            <div className="text-xs uppercase tracking-wider text-muted-foreground">Profiles</div>
            {profiles.length === 0 && (
              <div className="text-xs text-muted-foreground/70">no profiles yet</div>
            )}
            {profiles.map(p => (
              <div
                key={p.id}
                className="group flex items-center justify-between rounded border border-border/60 px-2 py-1.5 text-sm"
              >
                <button
                  type="button"
                  className="min-w-0 flex-1 truncate text-left hover:text-primary"
                  onClick={() => void loadOne(p.id)}
                  title={p.id}
                >
                  {p.display_name || p.id}
                </button>
                <button
                  type="button"
                  aria-label={`delete ${p.id}`}
                  className="ml-2 hidden text-muted-foreground hover:text-destructive group-hover:block"
                  onClick={() => void remove(p.id)}
                >
                  <Codicon name="trash" className="size-3.5" />
                </button>
              </div>
            ))}
          </div>

          {/* Right: editor */}
          <div className="flex min-w-0 flex-1 flex-col gap-3">
            <div className="flex items-center gap-2">
              <input
                className="min-w-0 flex-1 rounded border border-border/60 bg-transparent px-2 py-1.5 text-sm"
                placeholder="agent name (id)"
                value={name}
                onChange={e => setName(e.target.value)}
              />
              <span className="text-xs text-muted-foreground">id: {derivedId || '—'}</span>
              <Button onClick={() => void save()} disabled={busy || !derivedId}>
                {busy ? 'Saving…' : 'Save profile'}
              </Button>
            </div>
            {status && (
              <div className={status.kind === 'ok' ? 'text-xs text-emerald-500' : 'text-xs text-destructive'}>
                {status.text}
              </div>
            )}
            <textarea
              className="min-h-0 flex-1 resize-none rounded border border-border/60 bg-transparent p-3 font-mono text-xs leading-relaxed"
              spellCheck={false}
              value={yaml}
              onChange={e => setYaml(e.target.value)}
            />
            <div className="text-xs text-muted-foreground/70">
              schema_version 1 · saved to the Steward daemon; the kernel spawns this profile for
              matching subagent work.
            </div>
          </div>
      </div>
    </Panel>
  )
}
