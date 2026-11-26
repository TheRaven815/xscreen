param([string]$Screenshot)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class XscreenSmoke {
    public delegate bool Callback(IntPtr hwnd, IntPtr data);
    public delegate bool MonitorCallback(IntPtr monitor, IntPtr dc, ref Rect rect, IntPtr data);
    [DllImport("user32.dll")] public static extern bool EnumDisplayMonitors(IntPtr dc, IntPtr clip, MonitorCallback callback, IntPtr data);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr GetWindowDpiAwarenessContext(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool AreDpiAwarenessContextsEqual(IntPtr first, IntPtr second);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr context);
    public static Rect[] Monitors() {
        var result = new System.Collections.Generic.List<Rect>();
        EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, (IntPtr monitor, IntPtr dc, ref Rect rect, IntPtr data) => { result.Add(rect); return true; }, IntPtr.Zero);
        return result.ToArray();
    }
    [DllImport("user32.dll")] public static extern bool EnumWindows(Callback callback, IntPtr data);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint message, UIntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr dc, uint flags);
    public struct Rect { public int Left, Top, Right, Bottom; }
}
'@
[void][XscreenSmoke]::SetProcessDpiAwarenessContext([IntPtr]::new(-4))
function Get-AppWindows($ProcessId) {
    $windows = [System.Collections.Generic.List[object]]::new()
    [XscreenSmoke]::EnumWindows({
        param($hwnd, $data)
        [uint32]$owner = 0
        [void][XscreenSmoke]::GetWindowThreadProcessId($hwnd, [ref]$owner)
        if ($owner -eq $ProcessId) {
            $title = [System.Text.StringBuilder]::new(256)
            [void][XscreenSmoke]::GetWindowText($hwnd, $title, 256)
            $class = [System.Text.StringBuilder]::new(256)
            [void][XscreenSmoke]::GetClassName($hwnd, $class, 256)
            $windows.Add([pscustomobject]@{ Handle=$hwnd; Title=$title.ToString(); Class=$class.ToString(); Visible=[XscreenSmoke]::IsWindowVisible($hwnd) })
        }
        return $true
    }, [IntPtr]::Zero) | Out-Null
    return $windows
}
$app = Start-Process "$PSScriptRoot\..\target\debug\xscreen.exe" -PassThru
try {
    Start-Sleep -Seconds 3
    if ($app.HasExited) { throw 'App exited during startup.' }
    $windows = Get-AppWindows $app.Id
    foreach ($window in @($windows | Where-Object { $_.Visible -and $_.Title -like 'xscreen*' })) {
        $bounds = [XscreenSmoke+Rect]::new()
        [void][XscreenSmoke]::GetWindowRect($window.Handle, [ref]$bounds)
        if ($bounds.Left -gt -10000) { throw 'Visible application window at startup.' }
    }
    $tray = $windows | Where-Object Class -eq 'xscreen.tray' | Select-Object -First 1
    if (!$tray) { throw 'Tray owner not found.' }
    # WM_HOTKEY: post the OS notification to the app's hidden window.
    [void][XscreenSmoke]::PostMessage($tray.Handle, 0x312, [UIntPtr]::new(1), [IntPtr]::Zero)
    Start-Sleep -Seconds 3
    $overlay = Get-AppWindows $app.Id | Where-Object { $_.Title -like 'xscreen*Capture' -and $_.Visible }
    if (!$overlay) { throw 'Capture window did not open.' }
    $monitors = [XscreenSmoke]::Monitors()
    if (@($overlay).Count -ne $monitors.Count) { throw 'Overlay count differs from monitor count.' }
    foreach ($window in @($overlay)) {
        $rect = [XscreenSmoke+Rect]::new()
        [void][XscreenSmoke]::GetWindowRect($window.Handle, [ref]$rect)
        if (!@($monitors | Where-Object { $_.Left -eq $rect.Left -and $_.Top -eq $rect.Top -and $_.Right -eq $rect.Right -and $_.Bottom -eq $rect.Bottom }).Count) {
            throw "Overlay physical bounds differ from monitor: $($rect.Left),$($rect.Top),$($rect.Right),$($rect.Bottom)"
        }
        $context = [XscreenSmoke]::GetWindowDpiAwarenessContext($window.Handle)
        if (![XscreenSmoke]::AreDpiAwarenessContextsEqual($context, [IntPtr]::new(-4))) { throw 'Overlay is not per-monitor DPI v2.' }
        Write-Output "Monitor overlay: $($rect.Left),$($rect.Top) $($rect.Right-$rect.Left)x$($rect.Bottom-$rect.Top), DPI $([XscreenSmoke]::GetDpiForWindow($window.Handle))"
    }
    if ($Screenshot) {
        Add-Type -AssemblyName System.Drawing
        $rect = [XscreenSmoke+Rect]::new()
        [void][XscreenSmoke]::GetWindowRect(@($overlay)[0].Handle, [ref]$rect)
        $bitmap = [System.Drawing.Bitmap]::new($rect.Right-$rect.Left, $rect.Bottom-$rect.Top)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try { $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size) }
        finally { $graphics.Dispose() }
        try { $bitmap.Save($Screenshot) } finally { $bitmap.Dispose() }
    }
    [void][XscreenSmoke]::PostMessage($tray.Handle, 0x312, [UIntPtr]::new(1), [IntPtr]::Zero)
    Start-Sleep -Seconds 1
    if (@(Get-AppWindows $app.Id | Where-Object { $_.Title -like 'xscreen*Capture' -and $_.Visible }).Count) {
        throw 'Second shortcut did not close the overlay.'
    }
    # Tray double-click event.
    [void][XscreenSmoke]::PostMessage($tray.Handle, 0x8001, [UIntPtr]::Zero, [IntPtr]::new(0x203))
    Start-Sleep -Seconds 2
    if (!@(Get-AppWindows $app.Id | Where-Object { $_.Title -like 'xscreen*Capture' -and $_.Visible }).Count) {
        throw 'Tray double-click did not open the capture view.'
    }
    [void][XscreenSmoke]::PostMessage($tray.Handle, 0x7e, [UIntPtr]::Zero, [IntPtr]::Zero)
    Start-Sleep -Seconds 1
    if (@(Get-AppWindows $app.Id | Where-Object { $_.Title -like 'xscreen*Capture' -and $_.Visible }).Count) {
        throw 'Display change did not cancel capture.'
    }
    [void][XscreenSmoke]::PostMessage($tray.Handle, 0x111, [UIntPtr]::new(11), [IntPtr]::Zero)
    Start-Sleep -Seconds 1
    if (!@(Get-AppWindows $app.Id | Where-Object { $_.Title -like '*Settings' -and $_.Visible }).Count) {
        throw 'Settings window did not open.'
    }
    [void][XscreenSmoke]::PostMessage($tray.Handle, 0x111, [UIntPtr]::new(12), [IntPtr]::Zero)
    if (!$app.WaitForExit(5000)) { throw 'Application did not exit cleanly.' }
    Write-Output 'PASS: startup, per-monitor overlays, physical bounds, DPI v2, cancellation, tray double-click, display change, settings, clean exit.'
} finally {
    if (!$app.HasExited) { Stop-Process -Id $app.Id }
}
