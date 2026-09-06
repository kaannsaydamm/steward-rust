# Wire Protocol v2 (Omega §34)

Transport-independent app-server protocol. Types live in `steward-wire`;
the daemon mounts the WebSocket transport at `/api/v2/wire`.

## Handshake

Every connection performs:

1. TCP/WebSocket upgrade (loopback origin check; native clients may omit Origin).
2. Client sends `ClientEnvelope::Hello(Hello)` — protocol version, instance
   id, **auth token**, capability flags.
3. Server validates token + version; replies `ServerEnvelope::Hello(Hello)`.
4. Wrong token or unsupported major → `ServerEnvelope::Error` then close.

## Envelopes

```text
ClientEnvelope: Hello | Command(AppCommand) | Ack(EventAck) | Ping
ServerEnvelope: Hello | Response(AppResponse) | Event(RunEventV2)
              | Snapshot(StateSnapshot) | Error(AppError) | Pong
```

Commands: `start_run`, `cancel_run`, `signal`, `resolve_approval`.
Events carry server-owned monotonic `sequence` per run (§13.4).

## Resume

Client reconnect sends `Ack { run_id, last_sequence }`; the daemon replays
`events_after(run_id, last_sequence)` or a compact `StateSnapshot` when
retention rolled past that sequence.

## Error taxonomy

`AppError { code, message, remediation?, retryable }` — codes are the
`StewardErrorKind` strings (§13.10). Clients never parse message strings.

## Encodings

- JSON (diagnostic, implemented) — identical semantics to the binary form.
- binary protobuf-over-WebSocket (P-006) — same envelope schema; codegen
  from `proto/v2` lands with the Desktop phase.
