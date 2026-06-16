# Scope: Modern Web UI

## Architecture
- Next.js/React web dashboard connecting to `steward-daemon` via gRPC-web (`grpc-web` and `google-protobuf` packages or `@grpc/grpc-js`).
- Uses Tailwind CSS for a sleek, hacker-aesthetic interface.
- Components: Task Execution, Daemon Status, Log/Memory Viewer.

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | Protobuf & gRPC-Web Client | Generate protobuf clients for web, setup gRPC-web connection to `127.0.0.1:50051`. Implement `Ping` and `ExecuteTask` calls. | none | PLANNED |
| 2 | UI Components & Styling | Build the hacker-aesthetic dashboard with components for Status, Task Submission, and Log/History viewing. | M1 | PLANNED |

## Interface Contracts
### Web ↔ Daemon
- **Endpoint**: `http://127.0.0.1:50051`
- **RPC**: `steward.StewardService.Ping`
- **RPC**: `steward.StewardService.ExecuteTask`

## Code Layout
- `web-ui/src/app/page.tsx`: Main dashboard
- `web-ui/src/lib/grpc.ts`: gRPC client logic
- `proto/steward.proto`: Protobuf definitions to be compiled for JS.
