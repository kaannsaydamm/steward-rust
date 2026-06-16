import { createChannel, createClient } from "nice-grpc-web";
import { StewardServiceDefinition } from "./proto/steward";

const channel = createChannel("http://127.0.0.1:50051");

export const stewardClient = createClient(
  StewardServiceDefinition,
  channel
);
