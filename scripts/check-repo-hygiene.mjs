#!/usr/bin/env node
// Repository hygiene gate.
//
// Fails when internal planning material (roadmaps, task ledgers, agent
// instructions, implementation plans) is present in the working tree or in
// reachable Git history. Public contributor and user documentation is allowed;
// see docs/ for those.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";

const ROOT = process.cwd();

// Basenames (case-insensitive) that must never live in the repository.
const FORBIDDEN_FILENAMES = [
  "roadmap.md",
  "roadmaps.md",
  "spec.md",
  "specs.md",
  "task.md",
  "tasks.md",
  "plan.md",
  "plans.md",
  "implementation_plan.md",
  "implementation-plan.md",
  "research.md",
  "progress.md",
  "status.md",
  "todo.md",
  "todos.md",
  "agent.md",
  "agents.md",
  "claude.md",
  "prompts.md",
  "master_spec.md",
  "master-spec.md",
];

// Directory names (case-insensitive) that must never live in the repository.
const FORBIDDEN_DIRECTORIES = ["specs", "spec", "plans", "roadmap", "tasks", "prompts", "agents"];

// Suffixes that mark a file as internal progress tracking.
const FORBIDDEN_SUFFIXES = ["_progress.md", "-progress.md", "_roadmap.md", "-roadmap.md", "_plan.md", "-plan.md", "_tasks.md", "-tasks.md"];

function isForbiddenPath(path) {
  const normalized = path.replace(/\\/g, "/").toLowerCase();
  const segments = normalized.split("/");
  const base = segments[segments.length - 1];
  if (FORBIDDEN_FILENAMES.includes(base)) return "internal planning filename";
  if (FORBIDDEN_SUFFIXES.some((suffix) => base.endsWith(suffix))) return "internal progress filename";
  for (const segment of segments.slice(0, -1)) {
    if (FORBIDDEN_DIRECTORIES.includes(segment)) return `internal planning directory "${segment}/"`;
  }
  return null;
}

function trackedFiles() {
  return execFileSync("git", ["ls-files"], { cwd: ROOT, encoding: "utf8" })
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
}

function historyFiles() {
  try {
    const output = execFileSync("git", ["log", "--all", "--pretty=format:", "--name-only"], {
      cwd: ROOT,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    });
    return [...new Set(output.split("\n").map((line) => line.trim()).filter(Boolean))];
  } catch {
    return [];
  }
}

const findings = [];
const seen = new Set();

for (const path of [...trackedFiles(), ...historyFiles()]) {
  if (seen.has(path)) continue;
  seen.add(path);
  const reason = isForbiddenPath(path);
  if (reason) findings.push(`${path} (${reason})`);
}

if (findings.length > 0) {
  console.error("Repository hygiene check failed. Internal planning material must stay outside the repository:\n");
  for (const finding of findings) console.error(`  - ${finding}`);
  console.error(
    "\nRemove these files, keep the internal master plan outside the Git worktree, and rewrite history if they were committed.",
  );
  process.exit(1);
}

if (existsSync(join(ROOT, "ROADMAP.md")) || existsSync(join(ROOT, "SPECS"))) {
  console.error("Repository hygiene check failed: internal planning material found on disk.");
  process.exit(1);
}

console.log(`Repository hygiene check passed (${seen.size} tracked/historical paths inspected).`);
