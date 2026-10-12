// Runs the Tauri CLI with Cargo on PATH.
//
// `npm run tauri` uses this instead of calling the `tauri` shim directly, so
// the command behaves the same whether or not rustup added ~/.cargo/bin (or
// %USERPROFILE%\.cargo\bin) to the environment the script inherits.
//
// The CLI is launched as `node tauri.js` rather than through the npm shim: on
// Windows that shim is a .cmd file, and Node refuses to spawn one without a
// shell (EINVAL), which in turn would mean quoting a command string by hand.
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import { homedir } from "node:os";
import { delimiter, join } from "node:path";

const require = createRequire(import.meta.url);

let cli;
try {
  cli = require.resolve("@tauri-apps/cli/tauri.js");
} catch {
  console.error("The Tauri CLI is not installed. Run `npm install` first.");
  process.exit(1);
}

const cargoBin = join(homedir(), ".cargo", "bin");

// Windows stores the variable as "Path"; Node's env is case-insensitive there,
// but fall back explicitly so the value is never lost.
const currentPath = process.env.PATH ?? process.env.Path ?? "";

const env = {
  ...process.env,
  PATH: [cargoBin, currentPath].filter(Boolean).join(delimiter),
};

const child = spawn(process.execPath, [cli, ...process.argv.slice(2)], {
  stdio: "inherit",
  env,
});

child.on("error", (error) => {
  console.error(`Could not start the Tauri CLI: ${error.message}`);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});
