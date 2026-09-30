$ErrorActionPreference = 'Stop'; $ProgressPreference = 'SilentlyContinue';
$root = 'C:\probe\cyril-s2hb-premises-cefc9206';
if (Test-Path $root)
{ throw 'Probe directory already exists; do not overwrite evidence' 
}
New-Item -ItemType Directory -Path $root | Out-Null;
$zip = Join-Path $root 'probe.zip';
Invoke-WebRequest -UseBasicParsing -Uri 'http://10.143.1.1:18973/probe.zip' -OutFile $zip;
if ((Get-FileHash -Algorithm SHA256 $zip).Hash.ToLowerInvariant() -ne 'cefc9206fe6fe6c9d0ede1048f0bef2d9e3bf2d8de289f9ecf510d2c62bed8ad')
{ throw 'Probe archive hash mismatch' 
}
Expand-Archive -Path $zip -DestinationPath $root;
$env:Path = 'C:\probe\cyril-w3-tools\git\cmd;C:\Users\resourcefs\.cargo\bin;' + $env:Path;
Set-Location $root;
& 'C:\probe\cyril-w3-tools\python\python.exe' .\probe_python_contract.py .\crtool.py;
if ($LASTEXITCODE -ne 0)
{ exit $LASTEXITCODE 
}
cmd /c '"C:\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1 && set' | ForEach-Object { if ($_ -match '^(.*?)=(.*)$')
  { Set-Item -Path "env:$($matches[1])" -Value $matches[2] 
  } };
& 'C:\Users\resourcefs\.cargo\bin\rustup.exe' run 1.94.0 rustc --edition=2024 .\probe_std_process.rs -o .\probe_std_process.exe;
if ($LASTEXITCODE -ne 0)
{ exit $LASTEXITCODE 
}
& .\probe_std_process.exe 'C:\probe\cyril-w3-tools\python\python.exe';
exit $LASTEXITCODE;
