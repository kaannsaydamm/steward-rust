import { createChannel, createClient } from "nice-grpc-web";
import { StewardServiceDefinition } from "./proto/steward";

declare global {
  interface Window {
    __STEWARD_RPC_URL__?: string;
  }
}

const DEFAULT_RPC_URL = "http://127.0.0.1:50051";

function currentRpcUrl(): string {
  if (typeof window === "undefined") {
    return DEFAULT_RPC_URL;
  }
  return window.__STEWARD_RPC_URL__ ?? DEFAULT_RPC_URL;
}

function createStewardClient(rpcUrl: string) {
  return createClient(
    StewardServiceDefinition,
    createChannel(rpcUrl)
  );
}

let cachedRpcUrl = DEFAULT_RPC_URL;
let cachedClient = createStewardClient(cachedRpcUrl);

export function getStewardClient() {
  const nextRpcUrl = currentRpcUrl();
  if (nextRpcUrl !== cachedRpcUrl) {
    cachedRpcUrl = nextRpcUrl;
    cachedClient = createStewardClient(cachedRpcUrl);
  }
  return cachedClient;
}

export const stewardClient = new Proxy(cachedClient, {
  get(_target, property, receiver) {
    const client = getStewardClient();
    const value = Reflect.get(client, property, receiver);
    if (typeof value === "function") {
      return value.bind(client);
    }
    return value;
  },
});
