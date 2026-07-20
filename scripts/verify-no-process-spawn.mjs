#!/usr/bin/env node
/**
 * Security gate: fail if production Rust source can spawn processes.
 * Allowed only in tests/docs with explicit allow markers is NOT supported —
 * production code under src-tauri/src must never spawn processes.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const SRC = path.join(ROOT, "src-tauri", "src");

const FORBIDDEN = [
  { re: /\bstd\s*::\s*process\b/, msg: "std::process" },
  { re: /\btokio\s*::\s*process\b/, msg: "tokio::process" },
  { re: /\bCommand\s*::\s*new\b/, msg: "Command::new" },
  { re: /\btauri_plugin_shell\b/, msg: "tauri_plugin_shell" },
  { re: /\bShellExecute\b/, msg: "ShellExecute" },
  { re: /\bCreateProcess\b/, msg: "CreateProcess" },
  { re: /\bstd\s*::\s*os\s*::\s*windows\s*::\s*process\b/, msg: "windows process" },
];

function walk(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(p, out);
    else if (entry.isFile() && p.endsWith(".rs")) out.push(p);
  }
  return out;
}

const files = walk(SRC);
const violations = [];

for (const file of files) {
  // Skip pure test modules that live under #[cfg(test)] files named *test*
  const rel = path.relative(ROOT, file).replace(/\\/g, "/");
  const content = fs.readFileSync(file, "utf8");
  for (const { re, msg } of FORBIDDEN) {
    if (re.test(content)) {
      violations.push(`${rel}: forbidden ${msg}`);
    }
  }
}

// Also check Cargo.toml for shell plugin
const cargoToml = path.join(ROOT, "src-tauri", "Cargo.toml");
if (fs.existsSync(cargoToml)) {
  const cargo = fs.readFileSync(cargoToml, "utf8");
  if (/tauri-plugin-shell/.test(cargo)) {
    violations.push("src-tauri/Cargo.toml: tauri-plugin-shell dependency is forbidden");
  }
}

if (violations.length) {
  console.error("SECURITY FAIL: process spawn capability detected");
  for (const v of violations) console.error("  -", v);
  process.exit(1);
}

console.log(`verify-no-process-spawn: OK (${files.length} rust files scanned)`);
