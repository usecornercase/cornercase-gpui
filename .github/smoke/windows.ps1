$ErrorActionPreference = 'Continue'
$out = New-Item -ItemType Directory -Force smoke-out
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class W {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out RECT r, int s);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  public static RECT Bounds(IntPtr h) { RECT r; DwmGetWindowAttribute(h, 9, out r, 16); return r; }
  public static void Down() { mouse_event(2, 0, 0, 0, UIntPtr.Zero); }
  public static void Up() { mouse_event(4, 0, 0, 0, UIntPtr.Zero); }
}
"@
function Note($text) { $text | Add-Content (Join-Path $out notes.txt) }
function Shot($name) {
  $b = [System.Windows.Forms.SystemInformation]::VirtualScreen
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
  $bmp.Save((Join-Path $out "$name.png"))
  $g.Dispose(); $bmp.Dispose()
}
$shell = New-Object -ComObject WScript.Shell
function Front($app) { $null = $shell.AppActivate($app.Id); Start-Sleep -Milliseconds 400 }
function Keys($app, $keys, $name, $wait = 3) {
  Front $app
  [System.Windows.Forms.SendKeys]::SendWait($keys)
  Start-Sleep $wait
  Shot $name
}
function Rect($app) { $app.Refresh(); [W]::Bounds($app.MainWindowHandle) }
function Click($x, $y, $count = 1) {
  [W]::SetCursorPos($x, $y); Start-Sleep -Milliseconds 300
  for ($i = 0; $i -lt $count; $i++) { [W]::Down(); Start-Sleep -Milliseconds 40; [W]::Up(); Start-Sleep -Milliseconds 60 }
}
function State($app, $name) {
  $r = Rect $app
  Note "$name : rect $($r.L),$($r.T)-$($r.R),$($r.B) zoomed=$([W]::IsZoomed($app.MainWindowHandle)) iconic=$([W]::IsIconic($app.MainWindowHandle))"
}
try { Set-DisplayResolution -Width 1920 -Height 1080 -Force -ErrorAction Stop } catch { Note "resolution: $_" }
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
Start-Process notepad
Start-Sleep 2
$demo = ((cornercase status --json | ConvertFrom-Json).projects | Where-Object { $_.path -eq 'C:\demo' } | Select-Object -First 1).id
Note "demo project: $demo"
$app = Start-Process target\debug\cornercase-gpui.exe -ArgumentList '--project', "$demo" -PassThru -RedirectStandardError (Join-Path $out stderr.txt) -RedirectStandardOutput (Join-Path $out stdout.txt)
Start-Sleep 25
State $app first
Shot first
Keys $app "echo hello from windows{ENTER}" typed
Keys $app "^%+e" files 5
Keys $app "^%+c" changes 5
Keys $app "{ESC}" escape
Keys $app "^+/" shortcuts
Keys $app "{ESC}" closed
Keys $app "^+t" newtab 6
Keys $app "%1" tab1
$r = Rect $app
Front $app
[W]::SetCursorPos($r.R - 69, $r.T + 22); Start-Sleep 3
Shot hover-maximise
Click ($r.R - 69) ($r.T + 22); Start-Sleep 3
State $app maximised; Shot maximised
$r = Rect $app
Click ($r.R - 69) ($r.T + 22); Start-Sleep 3
State $app restored; Shot restored
$r = Rect $app
$mid = [int](($r.L + $r.R) / 2)
Click $mid ($r.T + 22) 2; Start-Sleep 3
State $app double-clicked; Shot double-clicked
Click $mid ($r.T + 22) 2; Start-Sleep 3
State $app double-clicked-again
$r = Rect $app
[W]::SetCursorPos($mid, $r.T + 22); [W]::Down(); Start-Sleep -Milliseconds 200
for ($i = 1; $i -le 10; $i++) { [W]::SetCursorPos($mid - 15 * $i, $r.T + 22 + 10 * $i); Start-Sleep -Milliseconds 50 }
[W]::Up(); Start-Sleep 2
State $app dragged; Shot dragged
Keys $app "^%+b" browser 15
Get-Process msedge, chrome -ErrorAction SilentlyContinue | Select-Object Name, Id | Format-Table | Out-String | Set-Content (Join-Path $out browsers.txt)
Keys $app "{ESC}" browser-closed
Keys $app "^+," settings 5
Keys $app "{ESC}" settings-closed
$r = Rect $app
Click ($r.R - 161) ($r.T + 22); Start-Sleep 2
State $app minimised
Front $app; Start-Sleep 2
State $app back
cornercase status --json 2>&1 | Set-Content (Join-Path $out status-after.json)
Get-Process | Select-Object Name, Id, Path | Format-Table -AutoSize | Out-String -Width 300 | Set-Content (Join-Path $out processes.txt)
Get-ChildItem -Recurse "$env:LOCALAPPDATA\cornercase" -Filter *.log -ErrorAction SilentlyContinue | ForEach-Object { Copy-Item $_.FullName $out }
Keys $app "%{F4}" altf4 5
if ($app.HasExited) { "exited with $($app.ExitCode)" | Set-Content (Join-Path $out exit.txt) } else { "still running after Alt+F4" | Set-Content (Join-Path $out exit.txt); Stop-Process -Id $app.Id }
Start-Sleep 3
Get-Process msedge, chrome -ErrorAction SilentlyContinue | Select-Object Name, Id | Format-Table | Out-String | Set-Content (Join-Path $out browsers-after.txt)
