#!/usr/bin/env node

import { chmod, mkdir, rm, writeFile } from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import process from "node:process";

const version = "0.1.0";
const platform = process.platform;
const architecture = process.arch;

if (platform !== "win32" || architecture !== "x64") {
  console.error(`Steward currently ships a Windows x64 release; detected ${platform} ${architecture}.`);
  process.exit(1);
}

const dataRoot = process.env.STEWARD_HOME || join(homedir(), ".steward");
const runtimeRoot = join(dataRoot, "runtime", `v${version}`);
const executable = join(runtimeRoot, "steward.exe");

try {
  await ensureRuntime();
  const result = spawnSync(executable, process.argv.slice(2), {
    stdio: "inherit",
    env: process.env,
  });
  if (result.error) throw result.error;
  process.exit(result.status ?? 1);
} catch (reason) {
  const message = reason instanceof Error ? reason.message : String(reason);
  console.error(`Unable to start Steward: ${message}`);
  process.exit(1);
}

async function ensureRuntime() {
  const probe = spawnSync(executable, ["--version"], { stdio: "ignore" });
  if (probe.status === 0) return;

  const releaseUrl = process.env.STEWARD_RELEASE_URL
    || `https://github.com/kaannsaydamm/steward-agent/releases/download/v${version}/steward-windows-x64.zip`;
  const archive = join(tmpdir(), `steward-${process.pid}.zip`);
  await mkdir(dirname(runtimeRoot), { recursive: true });
  await rm(runtimeRoot, { recursive: true, force: true });
  await mkdir(runtimeRoot, { recursive: true });

  console.error(`Installing Steward v${version}...`);
  const response = await fetch(releaseUrl, { redirect: "follow" });
  if (!response.ok) {
    throw new Error(`download failed with HTTP ${response.status} from ${releaseUrl}`);
  }
  await writeFile(archive, new Uint8Array(await response.arrayBuffer()));
  const extracted = spawnSync("powershell.exe", [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-Command",
    "& { param($Archive, $Destination) Expand-Archive -LiteralPath $Archive -DestinationPath $Destination -Force }",
    archive,
    runtimeRoot,
  ], {
    stdio: "inherit",
  });
  await rm(archive, { force: true });
  if (extracted.status !== 0) {
    throw new Error("Windows archive extraction failed");
  }
  await chmod(executable, 0o755);
  const verified = spawnSync(executable, ["--version"], { stdio: "ignore" });
  if (verified.status !== 0) {
    throw new Error("downloaded Steward runtime is incomplete");
  }
}
