$sig = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe"
$thumb = (Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Select-Object -First 1).Thumbprint
Set-Location C:\Users\AJK53\keysys\client\src-tauri
$env:PATH = "C:\Program Files\Rust stable MSVC 1.100\bin;" + $env:PATH

for ($i = 0; $i -lt 300; $i++) {
    $out = cargo build --release 2>&1 | Out-String
    if ($out -match "could not execute process ``([^``]+)`` \(never executed\)") {
        $exe = $Matches[1]
        if (-not $exe.EndsWith(".exe")) { $exe += ".exe" }
        if (Test-Path $exe) {
            & $sig sign /fd sha256 /sha1 $thumb $exe | Out-Null
            Write-Output "[$i] signed: $exe"
        } else {
            Write-Output "[$i] missing: $exe"
        }
        continue
    }
    Write-Output "=== done after $i retries ==="
    Write-Output ($out -split "`n" | Select-Object -Last 10 | Out-String)
    break
}
