// Runs the Tauri CLI with Cargo on PATH.
//
// `npm run tauri` uses this instead of calling `tauri` directly so the command
// behaves the same whether or not rustup added ~/.cargo/bin (or %USERPROFILE%
// \.cargo\bin) to the environment the script inherits.
import { spawn } from "node:child_process";
import { homedir } from "node:os";
import { delimiter, join } from "node:path";

const cargoBin = join(homedir(), ".cargo", "bin");

// Windows stores the variable as "Path"; Node's env is case-insensitive there,
// but fall back explicitly so the value is never lost.
const currentPath = process.env.PATH ?? process.env.Path ?? "";

const env = {
  ...process.env,
  PATH: [cargoBin, currentPath].filter(Boolean).join(delimiter),
};

// npm installs a .cmd shim on Windows, which spawn needs to be told about.
const executable = process.platform === "win32" ? "tauri.cmd" : "tauri";

const child = spawn(executable, process.argv.slice(2), { stdio: "inherit", env });

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
