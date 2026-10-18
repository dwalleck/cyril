#!/bin/bash
ssh -tt -o BatchMode=yes -o LogLevel=ERROR resourcefs-win11 'Set-Location C:\probe\pwsh-live-ws; C:\probe\cyril-k8wc-7ae63f66\target\debug\cyril.exe --agent-engine kas; Write-Output "CYRIL-EXITED code=$LASTEXITCODE"; Start-Sleep -Seconds 600'
