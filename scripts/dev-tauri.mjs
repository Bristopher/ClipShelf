// Wrapper around `tauri dev` that picks a free port at runtime.
// Windows reserves random high-port ranges for Hyper-V/WSL (see
// `netsh interface ipv4 show excludedportrange protocol=tcp`), so a
// hardcoded port in tauri.conf.json / vite.config.ts can fail with EACCES.
//
// We probe for two consecutive free ports (main + HMR), export VITE_PORT
// for vite.config.ts to read, and override Tauri's devUrl via --config.

import net from "node:net";
import { spawn } from "node:child_process";

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

const [vitePort, hmrPort] = await findPair();
console.log(`[dev-tauri] vite=${vitePort} hmr=${hmrPort}`);

const configOverride = JSON.stringify({
  build: { devUrl: `http://${HOST}:${vitePort}` },
});

const isWin = process.platform === "win32";
const child = spawn("pnpm", ["tauri", "dev", "--config", configOverride], {
  stdio: "inherit",
  shell: isWin,
  env: {
    ...process.env,
    VITE_PORT: String(vitePort),
    VITE_HMR_PORT: String(hmrPort),
  },
});

child.on("exit", (code) => process.exit(code ?? 0));
