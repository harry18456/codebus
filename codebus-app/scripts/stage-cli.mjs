// Stage the release `codebus` CLI binary into src-tauri/bin-staging/ so that
// the Tauri bundler can pick it up as a `resources` entry and ship it inside
// the Windows installer (lands at <install dir>/bin/codebus.exe). Runs as the
// first half of `beforeBuildCommand` before `npm run build`.
//
// `tauri build` only compiles the app crate, NOT the workspace CLI, so without
// this step the `resources` source file would be missing and bundling fails.
// We copy into a path *inside* src-tauri (bin-staging/) on purpose: Tauri
// resolves `resources` source paths relative to src-tauri, and keeping the
// source under src-tauri avoids the `..`-escapes-project resolution pitfall.

import { execFileSync } from "node:child_process";
import { mkdirSync, copyFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..", "..");
const isWindows = process.platform === "win32";
const exeName = isWindows ? "codebus.exe" : "codebus";

const source = resolve(repoRoot, "target", "release", exeName);
const destDir = resolve(scriptDir, "..", "src-tauri", "bin-staging");
const dest = resolve(destDir, exeName);

console.log("[stage-cli] building release CLI (codebus-cli)...");
execFileSync("cargo", ["build", "-p", "codebus-cli", "--release"], {
  cwd: repoRoot,
  stdio: "inherit",
});

console.log(`[stage-cli] staging ${source} -> ${dest}`);
mkdirSync(destDir, { recursive: true });
copyFileSync(source, dest);
console.log("[stage-cli] done.");
