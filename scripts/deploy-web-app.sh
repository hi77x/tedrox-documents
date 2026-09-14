#!/usr/bin/env bash
# TEDROX Documents — deploy the web application to Tedrox Cloud.
# The API token is read from the environment; it is never written to disk.
set -euo pipefail

if [[ -z "${TEDROX_API_KEY:-}" ]]; then
  echo "Set the TEDROX_API_KEY environment variable before deploying." >&2
  exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(dirname "$script_dir")"
cli="$script_dir/tedrox.mjs"
project_name="TEDROX Documents Web"

if [[ ! -f "$cli" ]]; then
  echo "tedrox.mjs was not found at $cli" >&2
  exit 1
fi

cd "$root"

echo "Building the browser application…"
(cd apps/web-app && npm ci --no-fund --no-audit && npm run build)

echo "Authenticating with Tedrox Cloud…"
node "$cli" doctor --json

project_id="$(node "$cli" projects list --json \
  | node -e 'let data="";process.stdin.on("data",(c)=>data+=c).on("end",()=>{const parsed=JSON.parse(data);const match=(parsed.data.items||[]).find((item)=>item.name===process.argv[1]);process.stdout.write(match?match.id:"")})' "$project_name")"

if [[ -z "$project_id" ]]; then
  echo "Creating project $project_name…"
  project_id="$(node "$cli" projects create --name "$project_name" --json \
    | node -e 'let data="";process.stdin.on("data",(c)=>data+=c).on("end",()=>{process.stdout.write(JSON.parse(data).data.id)})')"
fi

node "$cli" link --project "$project_id"
node "$cli" deploy "apps/web-app/dist" --project "$project_id" --wait
echo "Web application deployed."
