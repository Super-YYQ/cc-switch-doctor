#!/usr/bin/env node
/**
 * Verify release version consistency across package sources and optional tag/input.
 *
 * Usage:
 *   node scripts/verify-release-version.mjs
 *   node scripts/verify-release-version.mjs --expected 0.1.2
 *   EXPECTED_VERSION=0.1.2 node scripts/verify-release-version.mjs
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");

function read(p) {
  return fs.readFileSync(path.join(ROOT, p), "utf8");
}

function parseArgExpected() {
  const idx = process.argv.indexOf("--expected");
  if (idx >= 0 && process.argv[idx + 1]) return process.argv[idx + 1].replace(/^v/, "");
  if (process.env.EXPECTED_VERSION) return process.env.EXPECTED_VERSION.replace(/^v/, "");
  return null;
}

const SEMVER = /^\d+\.\d+\.\d+$/;

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
  console.error("RELEASE FAIL: version mismatch across source files");
  for (const [k, v] of Object.entries(versions)) console.error(`  ${k}: ${v}`);
  process.exit(1);
}

const version = pkg.version;
if (!SEMVER.test(version)) {
  console.error(`RELEASE FAIL: version "${version}" is not x.y.z`);
  process.exit(1);
}

const expected = parseArgExpected();
if (expected) {
  if (!SEMVER.test(expected)) {
    console.error(`RELEASE FAIL: expected version "${expected}" is not x.y.z`);
    process.exit(1);
  }
  if (expected !== version) {
    console.error(`RELEASE FAIL: expected ${expected} but sources are at ${version}`);
    process.exit(1);
  }
}

console.log(`verify-release-version: OK (v${version})`);
