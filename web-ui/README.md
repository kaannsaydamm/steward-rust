# Steward Web Console

Optional local operator console for Steward. It connects directly to the loopback gRPC-Web endpoint at `http://127.0.0.1:50051`.

```powershell
npm install
npm run dev
```

Production verification:

```powershell
npm run lint
npm run build
npm run start
```

The web console is a presentation client. Workflow policy, persistence, tool approvals, audit, MCP lifecycle, and retention remain owned by the Rust daemon.
