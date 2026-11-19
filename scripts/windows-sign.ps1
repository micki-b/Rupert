# Signs one file with Azure Artifact Signing. `tauri build` calls it for the
# app, the uninstaller and the installers (bundle.windows.signCommand, set by
# scripts/release-config.py); the release workflow's "Windows signing setup"
# step provides SignTool, Microsoft's signing plug-in and its metadata.
#
# Sign-in has no password: GitHub's OIDC token for this run, trusted by the
# Azure app registration's federated credential (docs/RELEASING.md). Those
# tokens are short-lived and the build takes a while, so every file gets a
# fresh one. Needs AZURE_CLIENT_ID and AZURE_TENANT_ID.
param([Parameter(Mandatory = $true)][string]$File)
$ErrorActionPreference = 'Stop'

if (!$env:ACTIONS_ID_TOKEN_REQUEST_URL) { throw 'No GitHub OIDC token (the job needs id-token: write)' }
$token = Invoke-RestMethod -Uri "$($env:ACTIONS_ID_TOKEN_REQUEST_URL)&audience=api://AzureADTokenExchange" `
  -Headers @{ Authorization = "bearer $($env:ACTIONS_ID_TOKEN_REQUEST_TOKEN)" }
$tokenFile = Join-Path $env:RUNNER_TEMP 'azure-federated-token'
Set-Content -Path $tokenFile -Value $token.value -NoNewline
$env:AZURE_FEDERATED_TOKEN_FILE = $tokenFile

& $env:RUPERT_SIGNTOOL sign /v /fd SHA256 /tr http://timestamp.acs.microsoft.com /td SHA256 `
  /d Rupert /dlib $env:RUPERT_SIGN_DLIB /dmdf $env:RUPERT_SIGN_METADATA $File
exit $LASTEXITCODE
