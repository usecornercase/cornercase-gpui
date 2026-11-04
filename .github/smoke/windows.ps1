$ErrorActionPreference = 'Continue'
$out = New-Item -ItemType Directory -Force smoke-out
function Shot($name) {
  Add-Type -AssemblyName System.Windows.Forms, System.Drawing
  $b = [System.Windows.Forms.SystemInformation]::VirtualScreen
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
  $bmp.Save((Join-Path $out "$name.png"))
  $g.Dispose(); $bmp.Dispose()
}
function Windows($name) {
  Get-Process | Where-Object { $_.MainWindowHandle -ne 0 } |
    Select-Object Name, Id, MainWindowTitle | Format-Table -AutoSize | Out-String -Width 200 |
    Set-Content (Join-Path $out "$name-windows.txt")
}
$env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"
cornercase --version | Tee-Object (Join-Path $out version.txt)
git init -q C:\demo
Set-Content C:\demo\README.md "# demo"
git -C C:\demo add -A; git -C C:\demo -c user.email=a@b -c user.name=ci commit -qm init
cornercase open C:\demo 2>&1 | Tee-Object (Join-Path $out open.txt)
cornercase status --json 2>&1 | Set-Content (Join-Path $out status.json)
Start-Process notepad
Start-Sleep 2
$app = Start-Process target\debug\cornercase-gpui.exe -PassThru -RedirectStandardError (Join-Path $out stderr.txt) -RedirectStandardOutput (Join-Path $out stdout.txt)
Start-Sleep 25
Windows first
Shot first
Start-Sleep 20
Windows later
Shot later
cornercase status --json 2>&1 | Set-Content (Join-Path $out status-after.json)
Get-Process | Select-Object Name, Id, Path | Format-Table -AutoSize | Out-String -Width 300 | Set-Content (Join-Path $out processes.txt)
Get-ChildItem -Recurse "$env:LOCALAPPDATA\cornercase" -Filter *.log -ErrorAction SilentlyContinue | ForEach-Object { Copy-Item $_.FullName $out }
if ($app.HasExited) { "exited with $($app.ExitCode)" | Set-Content (Join-Path $out exit.txt) } else { Stop-Process -Id $app.Id }
