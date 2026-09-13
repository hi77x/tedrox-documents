# Capture a real screenshot of the running desktop window via PrintWindow.
param(
    [string]$Exe = "target\release\tedrox-documents-desktop.exe",
    [string]$Out = "assets\screenshots\home.png",
    [int]$WaitSeconds = 10,
    [int]$ClickX = -1,
    [int]$ClickY = -1,
    [int]$Click2X = -1,
    [int]$Click2Y = -1,
    [string]$TypeText = "",
    [string]$AppArgs = ""
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class TdxWin32 {
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int cmd);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool ScreenToClient(IntPtr hwnd, ref POINT point);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

$startParams = @{ FilePath = $Exe; PassThru = $true }
if ($AppArgs -ne "") { $startParams.ArgumentList = $AppArgs }
$proc = Start-Process @startParams
$deadline = (Get-Date).AddSeconds($WaitSeconds)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
    $proc.Refresh()
    if ($proc.MainWindowHandle -ne 0 -and $proc.MainWindowHandle -ne $null) { break }
}
$proc.Refresh()
$hwnd = $proc.MainWindowHandle
if ($hwnd -eq 0) { $proc.Kill(); throw "The application window did not appear." }

[void][TdxWin32]::ShowWindow($hwnd, 5)
[void][TdxWin32]::SetForegroundWindow($hwnd)
Start-Sleep -Seconds 8

function Send-Click([int]$offsetX, [int]$offsetY) {
    $rect = New-Object TdxWin32+RECT
    [void][TdxWin32]::GetWindowRect($hwnd, [ref]$rect)
    $point = New-Object TdxWin32+POINT
    $point.X = $rect.Left + $offsetX
    $point.Y = $rect.Top + $offsetY
    [void][TdxWin32]::ScreenToClient($hwnd, [ref]$point)
    $lparam = (($point.Y -shl 16) -bor ($point.X -band 0xFFFF))
    [void][TdxWin32]::PostMessage($hwnd, 0x0201, [IntPtr]1, [IntPtr]$lparam)
    Start-Sleep -Milliseconds 120
    [void][TdxWin32]::PostMessage($hwnd, 0x0202, [IntPtr]0, [IntPtr]$lparam)
    Start-Sleep -Seconds 1
}

if ($ClickX -ge 0 -and $ClickY -ge 0) {
    Send-Click $ClickX $ClickY
}

if ($Click2X -ge 0 -and $Click2Y -ge 0) {
    Send-Click $Click2X $Click2Y
}

if ($TypeText -ne "") {
    Add-Type -AssemblyName System.Windows.Forms
    [void][TdxWin32]::SetForegroundWindow($hwnd)
    Start-Sleep -Milliseconds 400
    [System.Windows.Forms.SendKeys]::SendWait($TypeText)
    Start-Sleep -Seconds 2
}

$rect = New-Object TdxWin32+RECT
[void][TdxWin32]::GetWindowRect($hwnd, [ref]$rect)
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top

$bitmap = New-Object System.Drawing.Bitmap($width, $height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$hdc = $graphics.GetHdc()
[void][TdxWin32]::PrintWindow($hwnd, $hdc, 2)
$graphics.ReleaseHdc($hdc)
$graphics.Dispose()

$directory = Split-Path -Parent $Out
if ($directory -and -not (Test-Path $directory)) { New-Item -ItemType Directory -Force -Path $directory | Out-Null }
$bitmap.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bitmap.Dispose()

$proc.Kill()
$proc.WaitForExit()
Write-Host "Saved $Out ($width x $height)"
