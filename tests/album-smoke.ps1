param(
    [string]$ScreenshotDirectory = "$PSScriptRoot\..\dist\smoke\album",
    [string]$BinaryPath = "$PSScriptRoot\..\target\debug\xscreen.exe"
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class AlbumSmoke {
    public delegate bool Callback(IntPtr hwnd, IntPtr data);
    [DllImport("user32.dll")] public static extern bool EnumWindows(Callback callback, IntPtr data);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint message, UIntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hwnd, ref Point pt);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern bool IsClipboardFormatAvailable(uint format);
    [DllImport("user32.dll")] public static extern uint GetClipboardSequenceNumber();
    [DllImport("user32.dll")] public static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdcBlt, uint nFlags);
    public struct Rect { public int Left, Top, Right, Bottom; }
    public struct Point { public int X, Y; }
}
'@
[void][AlbumSmoke]::SetProcessDpiAwarenessContext([IntPtr]::new(-4))
function Get-Windows($ProcessId) {
    $result = [System.Collections.Generic.List[object]]::new()
    [void][AlbumSmoke]::EnumWindows({
        param($hwnd, $data)
        [uint32]$owner = 0
        [void][AlbumSmoke]::GetWindowThreadProcessId($hwnd, [ref]$owner)
        if ($owner -eq $ProcessId) {
            $title = [System.Text.StringBuilder]::new(256)
            $class = [System.Text.StringBuilder]::new(256)
            [void][AlbumSmoke]::GetWindowText($hwnd, $title, 256)
            [void][AlbumSmoke]::GetClassName($hwnd, $class, 256)
            $result.Add([pscustomobject]@{ Handle=$hwnd; Title=$title.ToString(); Class=$class.ToString(); Visible=[AlbumSmoke]::IsWindowVisible($hwnd) })
        }
        return $true
    }, [IntPtr]::Zero)
    return $result
}
function Client-Point($Window, [int]$X, [int]$Y) {
    $pt = [AlbumSmoke+Point]::new()
    $pt.X = $X
    $pt.Y = $Y
    [void][AlbumSmoke]::ClientToScreen($Window, [ref]$pt)
    return $pt
}
function Click-Client($Window, [int]$X, [int]$Y) {
    [void][AlbumSmoke]::SetForegroundWindow($Window)
    $pt = Client-Point $Window $X $Y
    [void][AlbumSmoke]::SetCursorPos($pt.X, $pt.Y)
    Start-Sleep -Milliseconds 60
    [AlbumSmoke]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 70
    [AlbumSmoke]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 450
}
function Drag-Client($Window, [int]$X1, [int]$Y1, [int]$X2, [int]$Y2) {
    [void][AlbumSmoke]::SetForegroundWindow($Window)
    $p1 = Client-Point $Window $X1 $Y1
    $p2 = Client-Point $Window $X2 $Y2
    [void][AlbumSmoke]::SetCursorPos($p1.X, $p1.Y)
    Start-Sleep -Milliseconds 60
    [AlbumSmoke]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 80
    for ($s = 1; $s -le 6; $s++) {
        $ix = [int]($p1.X + ($p2.X - $p1.X) * $s / 6.0)
        $iy = [int]($p1.Y + ($p2.Y - $p1.Y) * $s / 6.0)
        [void][AlbumSmoke]::SetCursorPos($ix, $iy)
        Start-Sleep -Milliseconds 30
    }
    [AlbumSmoke]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 300
}
function Press-Key([byte]$Key) {
    [AlbumSmoke]::keybd_event($Key,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [AlbumSmoke]::keybd_event($Key,0,2,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 150
}
function Press-CtrlKey([byte]$Key) {
    [AlbumSmoke]::keybd_event(17,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    Press-Key $Key
    [AlbumSmoke]::keybd_event(17,0,2,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 250
}
function Capture-Window($Window, $Name) {
    [void][AlbumSmoke]::SetWindowPos($Window,[IntPtr]::new(-1),0,0,0,0,0x3)
    [void][AlbumSmoke]::SetForegroundWindow($Window)
    Start-Sleep -Milliseconds 350
    # Move cursor away from buttons so hover popups don't obscure static screenshots unless intended
    [void][AlbumSmoke]::SetCursorPos(40, 40)
    Start-Sleep -Milliseconds 120
    $rect = [AlbumSmoke+Rect]::new()
    [void][AlbumSmoke]::GetWindowRect($Window, [ref]$rect)
    $bitmap = [System.Drawing.Bitmap]::new([Math]::Max(1, $rect.Right-$rect.Left), [Math]::Max(1, $rect.Bottom-$rect.Top))
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        try {
            $graphics.CopyFromScreen($rect.Left,$rect.Top,0,0,$bitmap.Size)
        } catch {
            $hdc = $graphics.GetHdc()
            try { [void][AlbumSmoke]::PrintWindow($Window, $hdc, 2) } finally { $graphics.ReleaseHdc($hdc) }
        }
    } finally { $graphics.Dispose() }
    try { $bitmap.Save((Join-Path $ScreenshotDirectory "$Name.png")) } finally {
        $bitmap.Dispose()
        [void][AlbumSmoke]::SetWindowPos($Window,[IntPtr]::new(-2),0,0,0,0,0x3)
    }
}

$scratch = Join-Path ([IO.Path]::GetTempPath()) "xscreen-album-$([guid]::NewGuid().ToString('N'))"
$priorAppData = $env:APPDATA
$app = $null
try {
    New-Item -ItemType Directory -Force -Path "$scratch\xscreen", "$scratch\captures", $ScreenshotDirectory | Out-Null
    $env:APPDATA = $scratch
    @{ hotkey_key=123; hotkey_modifiers=6; save_dir="$scratch\captures"; greeted=$true } | ConvertTo-Json | Set-Content "$scratch\xscreen\config.json"
    $app = Start-Process $BinaryPath -PassThru
    Start-Sleep -Seconds 2
    $tray = Get-Windows $app.Id | Where-Object Class -eq 'xscreen.tray' | Select-Object -First 1
    if (!$tray) { throw 'Tray owner missing.' }

    # 1. Open Album (empty state)
    [void][AlbumSmoke]::PostMessage($tray.Handle,0x111,[UIntPtr]::new(15),[IntPtr]::Zero)
    Start-Sleep -Seconds 2
    $window = Get-Windows $app.Id | Where-Object { $_.Visible -and $_.Title -like 'xscreen —*' } | Select-Object -First 1
    if (!$window) { throw 'Studio window did not open.' }
    [void][AlbumSmoke]::SetWindowPos($window.Handle,[IntPtr]::Zero,140,80,1036,699,0x4)
    Start-Sleep -Milliseconds 400
    Capture-Window $window.Handle 'empty'

    # 2. Populate synthetic PNG fixtures (labeled synthetic, isolated in temp scratch)
    for ($i=0; $i -lt 10; $i++) {
        $bitmap = [System.Drawing.Bitmap]::new(760,460)
        $g = [System.Drawing.Graphics]::FromImage($bitmap)
        $bg = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(26+$i*3,31+$i*3,39+$i*4))
        $card = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(40+$i*2,46+$i*2,56+$i*3))
        $accent = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(14,112,192))
        $ink = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(228,232,238))
        $muted = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(115,125,140))
        $font = [System.Drawing.Font]::new('Segoe UI',18,[System.Drawing.FontStyle]::Bold)
        $subFont = [System.Drawing.Font]::new('Consolas',11)
        try {
            $g.FillRectangle($bg,0,0,760,460)
            $g.FillRectangle($card,28,28,704,404)
            $g.FillRectangle($accent,28,28,704,6)
            $g.DrawString("Synthetic UI Capture #$($i+1)",$font,$ink,52,54)
            $g.DrawString("760x460 · test fixture · build $($100+$i)",$subFont,$muted,54,92)
            for ($r=0; $r -lt 4; $r++) { $g.FillRectangle($muted,54,142+$r*58,620-$r*85,22) }
            $bitmap.Save("$scratch\captures\2026-11-17_22-10-0$i.png")
        } finally { $subFont.Dispose(); $font.Dispose(); $accent.Dispose(); $card.Dispose(); $bg.Dispose(); $ink.Dispose(); $muted.Dispose(); $g.Dispose(); $bitmap.Dispose() }
    }

    # Trigger Album refresh via tray command
    [void][AlbumSmoke]::PostMessage($tray.Handle,0x111,[UIntPtr]::new(15),[IntPtr]::Zero)
    Start-Sleep -Seconds 2
    Capture-Window $window.Handle 'desktop'

    # 3. Click first tile -> opens in ShareX-style In-App Image Editor
    Click-Client $window.Handle 115 120
    Start-Sleep -Seconds 1
    $window = Get-Windows $app.Id | Where-Object { $_.Visible -and $_.Title -like '*Editor' } | Select-Object -First 1
    if (!$window) { throw 'Clicking tile did not open Editor.' }

    # Draw an Arrow (default tool) and a Rectangle (R), then switch to Select (V) and click the Rectangle to show handles!
    Drag-Client $window.Handle 340 260 520 340
    Press-Key 0x52 # 'R' -> Rectangle tool
    Drag-Client $window.Handle 440 230 680 410
    Press-Key 0x56 # 'V' -> Select tool
    Click-Client $window.Handle 440 320 # Click left edge of drawn rectangle to select it!
    Capture-Window $window.Handle 'editor'

    # Copy edited image to clipboard via Ctrl+C
    $clipboardSequence = [AlbumSmoke]::GetClipboardSequenceNumber()
    Press-CtrlKey 0x43 # Ctrl+C
    Start-Sleep -Milliseconds 800
    if ([AlbumSmoke]::GetClipboardSequenceNumber() -eq $clipboardSequence) { throw 'Editor Ctrl+C did not update clipboard.' }
    if (![AlbumSmoke]::IsClipboardFormatAvailable(17)) { throw 'Copy did not provide CF_DIBV5.' }

    # Save Copy via Save Copy button or Ctrl+S overwrite
    Press-CtrlKey 0x53 # Ctrl+S (overwrite save)
    Start-Sleep -Milliseconds 800

    # 4. Open Settings via tray command (11)
    [void][AlbumSmoke]::PostMessage($tray.Handle,0x111,[UIntPtr]::new(11),[IntPtr]::Zero)
    Start-Sleep -Milliseconds 700
    $window = Get-Windows $app.Id | Where-Object { $_.Visible -and $_.Title -like '*Settings' } | Select-Object -First 1
    if (!$window) { throw 'Settings navigation failed.' }
    Capture-Window $window.Handle 'settings'

    # Toggle Light theme from top-right Sun/Moon icon button
    $crect = [AlbumSmoke+Rect]::new()
    [void][AlbumSmoke]::GetClientRect($window.Handle, [ref]$crect)
    $cw = $crect.Right - $crect.Left
    # Right side layout: [Capture ~80px] [Theme 26px] [Lang 34px] with 10px margin & 5px spacing
    Click-Client $window.Handle ($cw - 103) 19
    $config = Get-Content "$scratch\xscreen\config.json" -Raw | ConvertFrom-Json
    if (!$config.light_theme) { throw 'Theme toggle did not persist.' }
    Capture-Window $window.Handle 'light'

    # Toggle Language (EN -> TR) from top-right EN/TR chip
    Click-Client $window.Handle ($cw - 138) 19
    $config = Get-Content "$scratch\xscreen\config.json" -Raw | ConvertFrom-Json
    if ($config.lang -ne 'tr') { throw "Language toggle did not persist (got $($config.lang))." }

    # Capture Turkish Editor in Dark mode too!
    Click-Client $window.Handle ($cw - 103) 19 # back to Dark mode
    [void][AlbumSmoke]::PostMessage($tray.Handle,0x111,[UIntPtr]::new(16),[IntPtr]::Zero) # Open Editor
    Start-Sleep -Milliseconds 600
    Capture-Window $window.Handle 'turkish-editor'

    # Compact window check (696x499)
    [void][AlbumSmoke]::PostMessage($tray.Handle,0x111,[UIntPtr]::new(15),[IntPtr]::Zero) # Album
    [void][AlbumSmoke]::SetWindowPos($window.Handle,[IntPtr]::Zero,140,80,696,499,0x4)
    Start-Sleep -Milliseconds 700
    Capture-Window $window.Handle 'compact'

    # 5. Test Capture button from top-right -> Escape returns to main window
    [void][AlbumSmoke]::SetWindowPos($window.Handle,[IntPtr]::Zero,140,80,1036,699,0x4)
    Start-Sleep -Milliseconds 500
    [void][AlbumSmoke]::GetClientRect($window.Handle, [ref]$crect)
    $cw = $crect.Right - $crect.Left
    Click-Client $window.Handle ($cw - 45) 19
    Start-Sleep -Seconds 2
    if (!@(Get-Windows $app.Id | Where-Object { $_.Title -like '*Capture' -and $_.Visible }).Count) { throw 'Capture button failed.' }
    Press-Key 27 # Escape
    Start-Sleep -Seconds 1
    $window = Get-Windows $app.Id | Where-Object { $_.Visible -and $_.Title -like 'xscreen —*' } | Select-Object -First 1
    if (!$window) { throw 'Studio failed to return after Escape.' }

    # Close window -> stays in tray
    [void][AlbumSmoke]::PostMessage($window.Handle,0x10,[UIntPtr]::Zero,[IntPtr]::Zero)
    Start-Sleep -Seconds 1
    if (@(Get-Windows $app.Id | Where-Object { $_.Visible -and $_.Title -like 'xscreen —*' }).Count) { throw 'Close did not hide window.' }
    if ($app.HasExited) { throw 'Closing window quit application.' }

    # Clean exit
    [void][AlbumSmoke]::PostMessage($tray.Handle,0x111,[UIntPtr]::new(12),[IntPtr]::Zero)
    if (!$app.WaitForExit(5000)) { throw 'App failed clean exit.' }
    Write-Output 'PASS: empty, album, ShareX editor (draw + select handles + copy + save), settings, light theme, Turkish localization, compact layout, capture & return, close-to-tray, clean exit.'
} finally {
    $env:APPDATA = $priorAppData
    if ($app -and !$app.HasExited) { Stop-Process -Id $app.Id }
    if (Test-Path $scratch) { Remove-Item -Recurse -Force $scratch }
}
