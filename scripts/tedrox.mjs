#!/usr/bin/env node
/** Tedrox CLI — dependency-free static-site deployment for Node.js 20+. */
import { readFile, readdir, stat, writeFile } from "node:fs/promises"
import { basename, join, relative, resolve, sep } from "node:path"

const VERSION = "1.1.0"
const DEFAULT_API = "https://app.tedrox.space"

function parseArgs(argv) {
  const args = { _: [] }
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index]
    if (!token.startsWith("--")) {
      args._.push(token)
      continue
    }
    const [rawKey, inline] = token.slice(2).split(/=(.*)/s, 2)
    const key = rawKey === "token" ? "api-key" : rawKey
    args[key] = inline ?? (argv[index + 1] && !argv[index + 1].startsWith("--") ? argv[++index] : true)
  }
  return args
}

function usage() {
  console.log(`Tedrox CLI ${VERSION}

Setup:
  curl -fsSLo tedrox.mjs https://app.tedrox.space/tedrox.mjs
  node tedrox.mjs link --project PROJECT_ID [--workspace WORKSPACE_ID]

Commands:
  node tedrox.mjs projects list [--workspace ID]
  node tedrox.mjs projects create --name "Landing" [--workspace ID]
  node tedrox.mjs project show [--project ID]
  node tedrox.mjs deploy [PATH] [--file PATH] [--project ID] [--wait]
  node tedrox.mjs deployments list [--project ID]
  node tedrox.mjs deployments status --deployment ID [--project ID]
  node tedrox.mjs link --project ID [--workspace ID]
  node tedrox.mjs doctor

Global options:
  --api URL         API origin (TEDROX_API or ${DEFAULT_API})
  --api-key TOKEN   API token (TEDROX_API_KEY or TEDROX_TOKEN)
  --workspace ID    Team workspace sent as X-Workspace-Id
  --json            Stable machine-readable output for scripts and AI agents
  --wait            Wait until a deployment is ready or failed

The link command writes .tedrox.json. Tokens are never written to that file.`)
}

function json(value) {
  process.stdout.write(`${JSON.stringify(value, null, 2)}\n`)
}

function fail(message, details, jsonMode = false) {
  if (jsonMode) {
    const error = { message }
    if (details) Object.assign(error, details)
    json({ ok: false, error })
  }
  else console.error(`Error: ${message}${details ? `\n${JSON.stringify(details, null, 2)}` : ""}`)
  process.exitCode = 1
}

async function readConfig() {
  try {
    const value = JSON.parse(await readFile(resolve(".tedrox.json"), "utf8"))
    return value && typeof value === "object" ? value : {}
  } catch (error) {
    if (error?.code === "ENOENT") return {}
    throw new Error(`Cannot read .tedrox.json: ${error instanceof Error ? error.message : String(error)}`)
  }
}

async function collectFiles(root) {
  const files = []
  const ignoredDirectories = new Set([".git", "node_modules", ".next", ".cache"])
  async function walk(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      if (entry.isDirectory() && ignoredDirectories.has(entry.name)) continue
      if ([".env", ".DS_Store", ".tedrox.json"].includes(entry.name)) continue
      const full = join(directory, entry.name)
      if (entry.isDirectory()) await walk(full)
      else if (entry.isFile()) files.push({ path: relative(root, full).split(sep).join("/"), bytes: await readFile(full) })
    }
  }
  await walk(root)
  return files
}

function createClient(args, config) {
  const api = String(args.api ?? process.env.TEDROX_API ?? config.api ?? DEFAULT_API).replace(/\/$/, "")
  const token = String(args["api-key"] ?? process.env.TEDROX_API_KEY ?? process.env.TEDROX_TOKEN ?? "")
  const workspace = String(args.workspace ?? process.env.TEDROX_WORKSPACE_ID ?? config.workspaceId ?? "")
  async function request(path, options = {}) {
    if (!token) throw new Error("Missing API token. Set TEDROX_API_KEY or pass --api-key.")
    const headers = new Headers(options.headers)
    headers.set("Authorization", `Bearer ${token}`)
    headers.set("User-Agent", `tedrox-cli/${VERSION}`)
    if (workspace) headers.set("X-Workspace-Id", workspace)
    const response = await fetch(`${api}${path}`, { ...options, headers })
    const body = await response.json().catch(() => null)
    if (!response.ok || !body?.ok) {
      const error = new Error(body?.error?.message || `Request failed with HTTP ${response.status}`)
      error.code = body?.error?.code || `HTTP_${response.status}`
      error.details = body?.error?.details
      throw error
    }
    return body.data
  }
  return { api, request, workspace }
}

async function waitForDeployment(client, projectId, deploymentId, jsonMode) {
  const started = Date.now()
  while (Date.now() - started < 10 * 60_000) {
    const data = await client.request(`/api/projects/${encodeURIComponent(projectId)}/deployments`)
    const deployment = data.items.find((item) => item.id === deploymentId)
    if (!deployment) throw new Error("Deployment disappeared from the project history.")
    if (!jsonMode) process.stderr.write(`\rDeployment ${deployment.status.padEnd(10)} v${deployment.sequence}`)
    if (deployment.status === "ready") {
      if (!jsonMode) process.stderr.write("\n")
      return deployment
    }
    if (deployment.status === "failed") throw Object.assign(new Error(deployment.error_message || "Deployment failed."), { code: deployment.error_code })
    await new Promise((done) => setTimeout(done, 2000))
  }
  throw new Error("Timed out after 10 minutes while waiting for deployment.")
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  const command = args._[0] ?? "help"
  if (["help", "-h"].includes(command) || args.help) return usage()
  if (command === "version" || args.version) return console.log(VERSION)

  const config = await readConfig()
  const client = createClient(args, config)
  const jsonMode = Boolean(args.json)
  const projectId = String(args.project ?? config.projectId ?? "")

  if (command === "link") {
    if (!projectId) throw new Error("Missing --project PROJECT_ID.")
    const next = { projectId, ...(client.workspace ? { workspaceId: client.workspace } : {}), ...(args.api ? { api: client.api } : {}) }
    await writeFile(resolve(".tedrox.json"), `${JSON.stringify(next, null, 2)}\n`, { flag: "w" })
    return jsonMode ? json({ ok: true, config: next }) : console.log(`Linked this directory to project ${projectId}.`)
  }

  if (command === "doctor") {
    const data = await client.request("/api/projects")
    const result = { api: client.api, authenticated: true, workspaceId: data.workspaceId, projects: data.items.length }
    return jsonMode ? json({ ok: true, data: result }) : console.log(`API: ${result.api}\nAuthentication: OK\nWorkspace: ${result.workspaceId}\nProjects: ${result.projects}`)
  }

  if (command === "projects") {
    const action = args._[1] ?? "list"
    if (action === "list") {
      const data = await client.request("/api/projects")
      if (jsonMode) return json({ ok: true, data })
      if (!data.items.length) return console.log("No projects in this workspace.")
      console.table(data.items.map(({ id, name, status, hostname }) => ({ id, name, status, hostname })))
      return
    }
    if (action === "create") {
      if (!args.name) throw new Error("Missing --name for the new project.")
      const data = await client.request("/api/projects", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ name: args.name }) })
      return jsonMode ? json({ ok: true, data }) : console.log(`Created ${data.name}\nProject: ${data.id}\nURL: https://${data.hostname}`)
    }
    throw new Error(`Unknown projects action: ${action}`)
  }

  if (command === "project" && (args._[1] ?? "show") === "show") {
    if (!projectId) throw new Error("Missing --project or .tedrox.json projectId.")
    const data = await client.request(`/api/projects/${encodeURIComponent(projectId)}`)
    return jsonMode ? json({ ok: true, data }) : console.log(`${data.name}\nID: ${data.id}\nStatus: ${data.status}\nURL: https://${data.hostname}`)
  }

  if (command === "deployments") {
    if (!projectId) throw new Error("Missing --project or .tedrox.json projectId.")
    const data = await client.request(`/api/projects/${encodeURIComponent(projectId)}/deployments`)
    const action = args._[1] ?? "list"
    if (action === "status") {
      const id = String(args.deployment ?? "")
      if (!id) throw new Error("Missing --deployment ID.")
      const deployment = data.items.find((item) => item.id === id)
      if (!deployment) throw new Error("Deployment not found.")
      return jsonMode ? json({ ok: true, data: deployment }) : console.log(`v${deployment.sequence} ${deployment.status}\n${deployment.id}`)
    }
    if (action !== "list") throw new Error(`Unknown deployments action: ${action}`)
    if (jsonMode) return json({ ok: true, data })
    console.table(data.items.map(({ id, sequence, status, source_name, created_at }) => ({ id, version: sequence, status, source: source_name, created: created_at })))
    return
  }

  if (command === "deploy") {
    if (!projectId) throw new Error("Missing --project or .tedrox.json projectId.")
    const source = String(args.file ?? args._[1] ?? ".")
    const info = await stat(source)
    const form = new FormData()
    if (info.isDirectory()) {
      const files = await collectFiles(source)
      if (!files.length) throw new Error("The deployment directory is empty.")
      for (const file of files) form.append("file", new Blob([file.bytes]), file.path)
    } else {
      const bytes = await readFile(source)
      const name = String(args.name ?? basename(source))
      const type = /\.html?$/i.test(name) ? "text/html" : "application/zip"
      form.append("file", new Blob([bytes], { type }), name)
    }
    const headers = {}
    if (args.idempotency) headers["Idempotency-Key"] = String(args.idempotency)
    const queued = await client.request(`/api/projects/${encodeURIComponent(projectId)}/deployments`, { method: "POST", headers, body: form })
    let deployment = queued
    if (args.wait) deployment = await waitForDeployment(client, projectId, queued.id, jsonMode)
    const result = { projectId, deployment, dashboardUrl: `${client.api}/dashboard/projects/${projectId}` }
    return jsonMode ? json({ ok: true, data: result }) : console.log(`Deployment v${deployment.sequence} ${deployment.status}.\nID: ${deployment.id}\nDashboard: ${result.dashboardUrl}`)
  }

  throw new Error(`Unknown command: ${command}`)
}

try {
  await main()
} catch (error) {
  const args = parseArgs(process.argv.slice(2))
  fail(error instanceof Error ? error.message : String(error), error?.code ? { code: error.code, ...(error.details ? { details: error.details } : {}) } : undefined, Boolean(args.json))
}
