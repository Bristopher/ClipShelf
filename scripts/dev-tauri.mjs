// Wrapper for the `tauri` CLI that picks free ports at runtime for `tauri dev`.
// Reason: Windows reserves random high-port ranges for Hyper-V/WSL (see
// `netsh interface ipv4 show excludedportrange protocol=tcp`), so a
// hardcoded port in tauri.conf.json / vite.config.ts can fail with EACCES.
//
// On `tauri dev` we probe for two free ports (main + HMR), write a
// temp override config (plain file path avoids Windows quote-mangling),
// export VITE_PORT / VITE_HMR_PORT for vite.config.ts to read, and run
// the real tauri CLI with `--config <tempfile>` appended.
// For any other subcommand (e.g. `tauri build`) we pass args through unchanged.

import net from "node:net";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ProjectRoot = path.resolve(__dirname, "..");

const HOST = "127.0.0.1";
const START = Number(process.env.DEV_PORT_START) || 5180;
const RANGE = 200;

function tryListen(port) {
  return new Promise((resolve) => {
    const s = net.createServer();
    s.once("error", () => resolve(false));
    s.once("listening", () => s.close(() => resolve(true)));
    s.listen(port, HOST);
  });
}

async function findPair() {
  for (let p = START; p < START + RANGE; p++) {
    if ((await tryListen(p)) && (await tryListen(p + 1))) return [p, p + 1];
  }
  throw new Error(`no free port pair found in ${START}..${START + RANGE}`);
}

const args = process.argv.slice(2);
const isDev = args[0] === "dev";
const env = { ...process.env };
const extra = [];

if (isDev) {
  const [vitePort, hmrPort] = await findPair();
  console.log(`[tauri] dev ports: vite=${vitePort} hmr=${hmrPort}`);

  const overridePath = path.join(ProjectRoot, "src-tauri", ".tauri.dev.json");
  fs.writeFileSync(
    overridePath,
    JSON.stringify({ build: { devUrl: `http://${HOST}:${vitePort}` } }),
  );

  extra.push("--config", overridePath);
  env.VITE_PORT = String(vitePort);
  env.VITE_HMR_PORT = String(hmrPort);
}

// Invoke the real tauri CLI. `pnpm exec` resolves the node_modules/.bin
// binary without re-entering our `tauri` npm script (which would recurse).
const child = spawn("pnpm", ["exec", "tauri", ...args, ...extra], {
  stdio: "inherit",
  shell: process.platform === "win32",
  env,
  cwd: ProjectRoot,
});

child.on("exit", (code) => process.exit(code ?? 0));
