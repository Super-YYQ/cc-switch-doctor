#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");

function read(p) {
  return fs.readFileSync(path.join(ROOT, p), "utf8");
}

const pkg = JSON.parse(read("package.json"));
const cargo = read("src-tauri/Cargo.toml");
const tauri = JSON.parse(read("src-tauri/tauri.conf.json"));
const manifest = JSON.parse(read("compatibility/manifest.json"));

const cargoMatch = cargo.match(/^version\s*=\s*"([^"]+)"/m);
if (!cargoMatch) {
  console.error("Could not parse version from Cargo.toml");
  process.exit(1);
}

const versions = {
  "package.json": pkg.version,
  "src-tauri/Cargo.toml": cargoMatch[1],
  "src-tauri/tauri.conf.json": tauri.version,
  "compatibility/manifest.json doctorVersion": manifest.doctorVersion,
};

const unique = new Set(Object.values(versions));
if (unique.size !== 1) {
  console.error("SECURITY/RELEASE FAIL: version mismatch");
  for (const [k, v] of Object.entries(versions)) console.error(`  ${k}: ${v}`);
  process.exit(1);
}

console.log(`verify-version-sync: OK (v${pkg.version})`);
