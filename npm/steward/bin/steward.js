#!/usr/bin/env node
/**
 * steward — unified CLI launcher.
 *
 *   steward [args…]        interactive coding agent (OMP-derived TUI) wired
 *                          to the local Steward daemon
 *   steward web [--port N] start the web dashboard and open the browser
 *   steward desktop        download/launch the desktop installer
 *   steward daemon         start the Steward daemon in the foreground
 *   steward --version      print the launcher version
 *
 * Resolution order for the runtime assets:
 *   1. STEWARD_HOME env (an existing checkout or install)
 *   2. ~/.steward (daemon config home; bin/ subdir for installed runtimes)
 *   3. GitHub releases of kaannsaydamm/steward-agent (download on demand)
 */
"use strict";

const { spawn } = require("node:child_process");
const fs = require("node:fs");
const https = require("node:https");
const os = require("node:os");
const path = require("node:path");

const VERSION = require("../package.json").version;
const REPO = "kaannsaydamm/steward-agent";

function configHome() {
  // Daemon state (wire-port, providers, db) always lives in the user config
  // home; STEWARD_HOME is the runtime/checkout root, not the config home.
  return process.env.STEWARD_CONFIG_HOME || path.join(os.homedir(), ".steward");
}

function stewardHome() {
  return process.env.STEWARD_HOME || configHome();
}

function fail(msg) {
  process.stderr.write(`steward: ${msg}\n`);
  process.exit(1);
}

function fetchBuffer(url, redirects = 0) {
  return new Promise((resolve, reject) => {
    if (redirects > 5) return reject(new Error("too many redirects"));
    https
      .get(url, { headers: { "user-agent": `steward-cli/${VERSION}` } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          return resolve(fetchBuffer(res.headers.location, redirects + 1));
        }
        if (res.statusCode !== 200) {
          res.resume();
          return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
        }
        const chunks = [];
        res.on("data", (c) => chunks.push(c));
        res.on("end", () => resolve(Buffer.concat(chunks)));
        res.on("error", reject);
      })
      .on("error", reject);
  });
}

async function downloadTo(url, dest) {
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  const buf = await fetchBuffer(url);
  fs.writeFileSync(dest, buf, { mode: 0o755 });
  return dest;
}

/** Locate the steward-daemon executable. */
function findDaemon() {
  const exe = process.platform === "win32" ? "steward-daemon.exe" : "steward-daemon";
  const candidates = [
    path.join(stewardHome(), "bin", exe),
    path.join(stewardHome(), exe),
  ];
  for (const c of candidates) if (fs.existsSync(c)) return c;
  return null;
}

/** Locate the agent CLI runtime (bun single-file or script). */
function findAgentCli() {
  const exe = process.platform === "win32" ? "steward-cli.exe" : "steward-cli";
  const home = stewardHome();
  const direct = [path.join(home, "bin", exe), path.join(home, exe)];
  for (const c of direct) if (fs.existsSync(c)) return c;
  // Dev checkout fallback (repo layout).
  const devScript = path.join(
    home,
    "vendor",
    "omp",
    "packages",
    "coding-agent",
    "src",
    "cli.ts",
  );
  if (fs.existsSync(devScript)) return devScript;
  return null;
}

function spawnPassthrough(cmd, args, opts = {}) {
  const child = spawn(cmd, args, {
    stdio: "inherit",
    windowsHide: false,
    ...opts,
  });
  child.on("exit", (code) => process.exit(code ?? 1));
  return child;
}

async function daemonAlreadyServing() {
  // A daemon advertises its wire port in ~/.steward/wire-port.
  const wirePortFile = path.join(configHome(), "wire-port");
  try {
    const port = parseInt(fs.readFileSync(wirePortFile, "utf8").trim(), 10);
    if (!port) return false;
    return await new Promise((resolve) => {
      const socket = require("node:net").connect(port, "127.0.0.1", () => {
        socket.destroy();
        resolve(true);
      });
      socket.on("error", () => resolve(false));
      socket.setTimeout(1500, () => {
        socket.destroy();
        resolve(false);
      });
    });
  } catch {
    return false;
  }
}

async function ensureDaemon() {
  if (await daemonAlreadyServing()) return null; // already running; nothing to launch
  const daemon = findDaemon();
  if (daemon) return daemon;
  process.stderr.write("steward: downloading steward-daemon from GitHub releases...\n");
  const asset =
    process.platform === "win32"
      ? "steward-daemon-windows-x64.exe"
      : `steward-daemon-${process.platform}-${process.arch}`;
  const url = `https://github.com/${REPO}/releases/latest/download/${asset}`;
  const dest = path.join(stewardHome(), "bin", process.platform === "win32" ? "steward-daemon.exe" : "steward-daemon");
  try {
    await downloadTo(url, dest);
  } catch (err) {
    fail(`could not download daemon (${err.message}). Install manually from https://github.com/${REPO}/releases`);
  }
  return dest;
}

async function cmdWeb(argv) {
  const portIdx = argv.indexOf("--port");
  const port = portIdx !== -1 ? argv[portIdx + 1] : "8093";
  const daemon = await ensureDaemon();

  // Daemon up? Read the persisted wire port; otherwise start one.
  const wirePortFile = path.join(configHome(), "wire-port");
  let daemonProc = null;
  if (!fs.existsSync(wirePortFile)) {
    daemonProc = spawn(daemon, ["--port", "50051", "--web-port", "3000"], {
      stdio: "ignore",
      detached: false,
    });
    // Wait for the wire-port file to appear (up to 30 s).
    const deadline = Date.now() + 30_000;
    while (!fs.existsSync(wirePortFile) && Date.now() < deadline) {
      await new Promise((r) => setTimeout(r, 300));
    }
  }

  const python = process.platform === "win32" ? "python.exe" : "python3";
  const hermesRoot = path.join(stewardHome(), "vendor", "hermes");
  const webServer = path.join(hermesRoot, "hermes_cli", "web_server.py");
  if (fs.existsSync(webServer)) {
    // Dev checkout: run the vendored dashboard (serves the built SPA + RPC).
    const child = spawn(python, ["-m", "hermes_cli.main", "dashboard", "--port", port, "--skip-build"], {
      cwd: hermesRoot,
      stdio: "inherit",
      env: { ...process.env, HERMES_HOME: process.env.HERMES_HOME || stewardHome() },
    });
    await new Promise((r) => setTimeout(r, 4_000));
    openBrowser(`http://127.0.0.1:${port}/chat`);
    child.on("exit", () => {
      if (daemonProc) daemonProc.kill();
      process.exit(0);
    });
  } else {
    // Installed layout: the daemon serves the WebUI on --web-port directly.
    const webPort = parseInt(port, 10) || 8093;
    spawnPassthrough(daemon, ["--port", "50051", "--web-port", String(webPort)]);
    await new Promise((r) => setTimeout(r, 4_000));
    openBrowser(`http://127.0.0.1:${webPort}`);
  }
}

function openBrowser(url) {
  const start =
    process.platform === "win32"
      ? "cmd"
      : process.platform === "darwin"
        ? "open"
        : "xdg-open";
  const args = process.platform === "win32" ? ["/c", "start", "", url] : [url];
  spawn(start, args, { stdio: "ignore", detached: true }).unref();
}

async function main() {
  const argv = process.argv.slice(2);
  const command = argv[0];

  if (command === "--version" || command === "-v") {
    process.stdout.write(`steward/${VERSION}\n`);
    return;
  }

  if (command === "daemon") {
    const daemon = await ensureDaemon();
    spawnPassthrough(daemon, argv.slice(1));
    return;
  }

  if (command === "web") {
    await cmdWeb(argv.slice(1));
    return;
  }

  if (command === "desktop") {
    const asset =
      process.platform === "win32"
        ? "steward-setup-windows-x64.exe"
        : `steward-${process.platform}-${process.arch}.${process.platform === "darwin" ? "dmg" : "AppImage"}`;
    const url = `https://github.com/${REPO}/releases/latest/download/${asset}`;
    const dest = path.join(os.tmpdir(), asset);
    process.stderr.write(`steward: downloading desktop installer...\n`);
    try {
      await downloadTo(url, dest);
    } catch (err) {
      fail(`could not download desktop installer (${err.message}). Get it from https://github.com/${REPO}/releases`);
    }
    spawn(process.platform === "win32" ? "cmd" : dest, process.platform === "win32" ? ["/c", "start", "", dest] : [], { stdio: "ignore", detached: true }).unref();
    return;
  }

  // Default: the interactive agent CLI.
  const cli = findAgentCli();
  if (!cli) {
    fail(`agent runtime not found under ${stewardHome()}. Install it or set STEWARD_HOME to your Steward checkout.`);
  }
  if (cli.endsWith(".ts")) {
    const bun = process.platform === "win32"
      ? path.join(process.env.LOCALAPPDATA || "", "bun-bin", "bun.exe")
      : "bun";
    spawnPassthrough(fs.existsSync(bun) ? bun : "bun", [cli, ...argv]);
  } else {
    spawnPassthrough(cli, argv);
  }
}

main().catch((err) => fail(err.message));
