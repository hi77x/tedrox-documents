# TEDROX Documents — deploy the landing to Tedrox Cloud (Windows PowerShell).
# The API token is read from the environment; it is never written to disk.
$ErrorActionPreference = "Stop"

if (-not $env:TEDROX_API_KEY) {
    Write-Error "Set the TEDROX_API_KEY environment variable before deploying."
    exit 1
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = Split-Path -Parent $scriptDir
$cli = Join-Path $scriptDir "tedrox.mjs"
$projectName = "TEDROX Documents"

if (-not (Test-Path $cli)) {
    Write-Error "tedrox.mjs was not found at $cli"
    exit 1
}

Push-Location $root
try {
    node (Join-Path $scriptDir "sync-i18n.mjs")
    node (Join-Path $scriptDir "generate-releases.mjs")

    Write-Host "Authenticating with Tedrox Cloud…"
    node $cli doctor --json | Out-Host

    $projects = (node $cli projects list --json | ConvertFrom-Json).data.items
    $project = $projects | Where-Object { $_.name -eq $projectName } | Select-Object -First 1
    if (-not $project) {
        Write-Host "Creating project $projectName…"
        $created = (node $cli projects create --name $projectName --json | ConvertFrom-Json).data
        $projectId = $created.id
        Write-Host "Project URL: https://$($created.hostname)"
    }
    else {
        $projectId = $project.id
        Write-Host "Reusing project $projectId (https://$($project.hostname))"
    }

    node $cli link --project $projectId
    node $cli deploy "apps/web" --project $projectId --wait
    Write-Host "Landing deployed."
}
finally {
    Pop-Location
}
