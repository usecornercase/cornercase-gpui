$ErrorActionPreference = 'Continue'
$out = New-Item -ItemType Directory -Force smoke-out
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
function Shot($name) {
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
$shell = New-Object -ComObject WScript.Shell
function Keys($app, $keys, $name, $wait = 3) {
  $null = $shell.AppActivate($app.Id)
  Start-Sleep -Milliseconds 400
  [System.Windows.Forms.SendKeys]::SendWait($keys)
  Start-Sleep $wait
  Shot $name
}
try { Set-DisplayResolution -Width 1920 -Height 1080 -Force -ErrorAction Stop } catch { "resolution: $_" | Add-Content (Join-Path $out notes.txt) }
[System.Windows.Forms.SystemInformation]::VirtualScreen | Out-String | Add-Content (Join-Path $out notes.txt)
$env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"
cornercase --version | Tee-Object (Join-Path $out version.txt)
git init -q C:\demo
Set-Content C:\demo\README.md "# demo"
New-Item -ItemType Directory -Force C:\demo\src | Out-Null
Set-Content C:\demo\src\main.rs "fn main() {`n    println!(""hello"");`n}"
git -C C:\demo add -A; git -C C:\demo -c user.email=a@b -c user.name=ci commit -qm init
Add-Content C:\demo\README.md "changed"
cornercase start-server 2>&1 | Set-Content (Join-Path $out start.txt)
Push-Location C:\demo
cornercase open C:\demo 2>&1 | Set-Content (Join-Path $out open.txt)
Pop-Location
cornercase status --json 2>&1 | Set-Content (Join-Path $out status.json)
Start-Process notepad
Start-Sleep 2
$app = Start-Process target\debug\cornercase-gpui.exe -PassThru -RedirectStandardError (Join-Path $out stderr.txt) -RedirectStandardOutput (Join-Path $out stdout.txt)
Start-Sleep 25
Windows first
Shot first
Keys $app "echo hello from windows{ENTER}" typed
Keys $app "dir src{ENTER}" dir
Keys $app "^+e" files 5
Keys $app "^+c" changes 5
Keys $app "{ESC}" escape
Keys $app "^+/" shortcuts
Keys $app "{ESC}" closed
Keys $app "^+t" newtab 6
Keys $app "%1" tab1
Keys $app "^+," settings 5
Keys $app "{ESC}" settings-closed
Windows later
cornercase status --json 2>&1 | Set-Content (Join-Path $out status-after.json)
Get-Process | Select-Object Name, Id, Path | Format-Table -AutoSize | Out-String -Width 300 | Set-Content (Join-Path $out processes.txt)
Get-ChildItem -Recurse "$env:LOCALAPPDATA\cornercase" -Filter *.log -ErrorAction SilentlyContinue | ForEach-Object { Copy-Item $_.FullName $out }
Keys $app "%{F4}" altf4 5
if ($app.HasExited) { "exited with $($app.ExitCode)" | Set-Content (Join-Path $out exit.txt) } else { "still running after Alt+F4" | Set-Content (Join-Path $out exit.txt); Stop-Process -Id $app.Id }
