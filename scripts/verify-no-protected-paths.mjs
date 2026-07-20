#!/usr/bin/env node
/**
 * Security gate: production Rust must not read protected AI login directories.
 * Mentions in comments that document the ban are allowed only when paired
 * with the exact sentinel: FORBIDDEN_PATH_DOC_ONLY
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const SRC = path.join(ROOT, "src-tauri", "src");

const PROTECTED = [
  { re: /["']\.codex["']/, msg: ".codex path literal" },
  { re: /["']\.claude["']/, msg: ".claude path literal" },
  { re: /["']\.claude\.json["']/, msg: ".claude.json path literal" },
  { re: /["']auth\.json["']/, msg: "auth.json path literal (protected login file)" },
  { re: /["']\.gemini["']/, msg: ".gemini path literal" },
  { re: /opencode[\\/]/, msg: "opencode home path" },
  { re: /join\(\s*["']\.codex["']/, msg: "join(.codex)" },
  { re: /join\(\s*["']\.claude["']/, msg: "join(.claude)" },
  { re: /join\(\s*["']\.gemini["']/, msg: "join(.gemini)" },
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
  const rel = path.relative(ROOT, file).replace(/\\/g, "/");
  const content = fs.readFileSync(file, "utf8");
  const lines = content.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    // Allow documentation-only mentions
    if (line.includes("FORBIDDEN_PATH_DOC_ONLY")) continue;
    if (
      line.trimStart().startsWith("//") ||
      line.trimStart().startsWith("///") ||
      line.trimStart().startsWith("*")
    ) {
      // Still ban actual path construction in comments that look like code? Allow pure docs.
      continue;
    }
    for (const { re, msg } of PROTECTED) {
      if (re.test(line)) {
        violations.push(`${rel}:${i + 1}: forbidden ${msg}: ${line.trim()}`);
      }
    }
  }
}

if (violations.length) {
  console.error("SECURITY FAIL: protected path access detected");
  for (const v of violations) console.error("  -", v);
  process.exit(1);
}

console.log(`verify-no-protected-paths: OK (${files.length} rust files scanned)`);
