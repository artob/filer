# This is free and unencumbered software released into the public domain.
#
# Runs job.ps1 from the shared folder, logging to job.log (UTF-8), then
# writes job.done with the exit status. An elevated task may not see the
# drive the share is mapped to, so the share is also tried by name.
$ErrorActionPreference = "Continue"
while ($true) {
    foreach ($share in @("Z:\", "\\host.lan\Data\")) {
        $job = Join-Path $share "job.ps1"
        if (Test-Path $job) {
            $running = Join-Path $share "job.running.ps1"
            Move-Item -Force $job $running
            $log = Join-Path $share "job.log"
            try {
                & powershell -NoProfile -ExecutionPolicy Bypass -File $running *>&1 |
                    Out-File -Encoding utf8 $log
                $status = $LASTEXITCODE
            } catch {
                $_ | Out-File -Append $log
                $status = 1
            }
            Set-Content -Path (Join-Path $share "job.done") -Value "$status"
            break
        }
    }
    Start-Sleep -Seconds 3
}
