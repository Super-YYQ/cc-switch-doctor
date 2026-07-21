#!/usr/bin/env node
/**
 * Fail if any GitHub Actions workflow uses an unpinned `uses:` reference.
 * Every third-party action must be pinned to a full 40-char commit SHA
 * (optional trailing comment with the human version tag is fine).
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const WF_DIR = path.join(ROOT, ".github", "workflows");

const SHA40 = /^[0-9a-f]{40}$/i;
// uses: owner/repo@sha  OR  uses: owner/repo/path@sha  OR  uses: docker://...
const USES_RE = /^\s*-?\s*uses:\s*['"]?([^'"\s#]+)['"]?/;

let failed = false;
const files = fs.existsSync(WF_DIR)
  ? fs.readdirSync(WF_DIR).filter((f) => f.endsWith(".yml") || f.endsWith(".yaml"))
  : [];

if (files.length === 0) {
  console.error("verify-actions-pinned: no workflow files found");
  process.exit(1);
}

for (const file of files) {
  const full = path.join(WF_DIR, file);
  const lines = fs.readFileSync(full, "utf8").split(/\r?\n/);
  lines.forEach((line, idx) => {
    const m = line.match(USES_RE);
    if (!m) return;
    const ref = m[1];
    // local actions (./) are ok
    if (ref.startsWith("./")) return;
    // docker:// images are out of scope for SHA pin of GitHub Actions
    if (ref.startsWith("docker://")) {
      console.error(`${file}:${idx + 1}: docker actions are not allowed without review: ${ref}`);
      failed = true;
      return;
    }
    const at = ref.lastIndexOf("@");
    if (at < 0) {
      console.error(`${file}:${idx + 1}: missing @ref: ${ref}`);
      failed = true;
      return;
    }
    const pin = ref.slice(at + 1);
    if (!SHA40.test(pin)) {
      console.error(`${file}:${idx + 1}: action not pinned to 40-char SHA: ${ref} (got "${pin}")`);
      failed = true;
    }
  });
}

if (failed) {
  console.error("verify-actions-pinned: FAIL");
  process.exit(1);
}
console.log(`verify-actions-pinned: OK (${files.length} workflows)`);
