# TEDROX Documents — deploy the landing to Tedrox Cloud (bash).
# The API token is read from the environment; it is never written to disk.
set -euo pipefail

if [ -z "${TEDROX_API_KEY:-}" ]; then
  echo "Set the TEDROX_API_KEY environment variable before deploying." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"
CLI="$SCRIPT_DIR/tedrox.mjs"
PROJECT_NAME="TEDROX Documents"

cd "$ROOT"
node "$SCRIPT_DIR/sync-i18n.mjs"
node "$SCRIPT_DIR/generate-releases.mjs"

node "$CLI" doctor --json
PROJECT_ID="$(node "$CLI" projects list --json | node -e '
let input = "";
process.stdin.on("data", (chunk) => (input += chunk));
process.stdin.on("end", () => {
  const data = JSON.parse(input);
  const project = data.data.items.find((item) => item.name === "TEDROX Documents");
  process.stdout.write(project ? project.id : "");
});
')"

if [ -z "$PROJECT_ID" ]; then
  PROJECT_ID="$(node "$CLI" projects create --name "$PROJECT_NAME" --json | node -e '
let input = "";
process.stdin.on("data", (chunk) => (input += chunk));
process.stdin.on("end", () => process.stdout.write(JSON.parse(input).data.id));
')"
fi

node "$CLI" link --project "$PROJECT_ID"
node "$CLI" deploy "apps/web" --project "$PROJECT_ID" --wait
echo "Landing deployed."
