# Limit physical input to the test process and the Edge window in its disposable profile.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class DesktopProbe {
    public struct Point { public int X, Y; }
    public delegate bool Visit(IntPtr hwnd, IntPtr data);
    [DllImport("user32")] public static extern bool EnumWindows(Visit visit, IntPtr data);
    [DllImport("user32")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder title, int length);
    [DllImport("user32")] public static extern bool ClientToScreen(IntPtr hwnd, ref Point point);
    [DllImport("user32")] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32")] public static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32")] public static extern IntPtr GetAncestor(IntPtr hwnd, uint flag);
    [DllImport("user32")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [DllImport("user32")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
    static IntPtr Find(uint app, string title, bool contains) {
        IntPtr window = IntPtr.Zero;
        EnumWindows((candidate, data) => {
            uint pid;
            GetWindowThreadProcessId(candidate, out pid);
            if (pid != app) return true;
            var name = new StringBuilder(512);
            GetWindowText(candidate, name, name.Capacity);
            if (contains ? !name.ToString().Contains(title) : name.ToString() != title) return true;
            window = candidate;
            return false;
        }, IntPtr.Zero);
        if (window == IntPtr.Zero) throw new InvalidOperationException("Missing test window: " + title);
        return window;
    }
    [DllImport("user32")] static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32")] static extern uint MapVirtualKey(uint code, uint mapType);
    static void Press(byte key, bool up) {
        uint flags = (up ? 2u : 0u) | (key == 35 || key == 36 ? 1u : 0u);
        keybd_event(key, (byte)MapVirtualKey(key, 0), flags, UIntPtr.Zero);
        System.Threading.Thread.Sleep(40);
    }
    public static void Keys(uint app, string title, bool contains, byte[] keys) {
        var window = Find(app, title, contains);
        for (int i = 0; i < 20 && GetForegroundWindow() != window; i++) System.Threading.Thread.Sleep(50);
        if (GetForegroundWindow() != window) throw new InvalidOperationException("Test window lacks keyboard focus: " + title);
        int held = 0;
        try {
            foreach (byte key in keys) {
                if (GetForegroundWindow() != window) throw new InvalidOperationException("Keyboard focus changed");
                Press(key, false);
                held++;
            }
        } finally { while (held > 0) Press(keys[--held], true); }
    }
    public static void Drag(uint app, string title, int x, int y, uint edge, string host) {
        SetThreadDpiAwarenessContext((IntPtr)(-4));
        var window = Find(app, title, false);
        var at = new Point { X = x, Y = y };
        if (window == IntPtr.Zero || !ClientToScreen(window, ref at) || GetAncestor(WindowFromPoint(at), 2) != window)
            throw new InvalidOperationException("The isolated document caption is not visible.");
        int destX = at.X + 58, destY = at.Y + 36;
        if (edge != 0) {
            var target = Find(edge, host, true);
            var area = System.Windows.Forms.Screen.FromHandle(window).WorkingArea;
            int left = at.X < area.Left + area.Width / 2 ? area.Right - 360 : area.Left;
            SetWindowPos(target, IntPtr.Zero, left, area.Top + 40, 360, 290, 0x10);
            destX = left + 160;
            destY = area.Top + 180;
            System.Threading.Thread.Sleep(200);
            if (GetAncestor(WindowFromPoint(new Point {X=destX, Y=destY}), 2) != target)
                throw new InvalidOperationException("Test Edge drop target is obscured.");
        }
        if (GetAncestor(WindowFromPoint(at), 2) != window) throw new InvalidOperationException("Drag source was obscured.");
        Point previous;
        GetCursorPos(out previous);
        try {
            SetCursorPos(at.X,at.Y);
            mouse_event(2,0,0,0,UIntPtr.Zero);
            System.Threading.Thread.Sleep(150);
            SetCursorPos(at.X+8,at.Y+6);
            System.Threading.Thread.Sleep(100);
            for (int i = 1; i <= 20; i++) {
                SetCursorPos(at.X + (destX-at.X)*i/20, at.Y + (destY-at.Y)*i/20);
                System.Threading.Thread.Sleep(24);
            }
            System.Threading.Thread.Sleep(150);
        } finally {
            mouse_event(4,0,0,0,UIntPtr.Zero);
            System.Threading.Thread.Sleep(900); // Let the application resolve the released pointer.
            SetCursorPos(previous.X,previous.Y);
        }
    }
}
"@
$inputAction = $env:JOT_QA_INPUT | ConvertFrom-Json
if ($inputAction.keys) {
    $targetPid = if ($inputAction.external) { $env:JOT_QA_EDGE } else { $env:JOT_QA_PID }
    $title = if ($inputAction.external) { $env:JOT_QA_HOST } else { $inputAction.title }
    $keys = [byte[]]@($inputAction.keys | ForEach-Object { [Enum]::Parse([System.Windows.Forms.Keys], $_) })
    [DesktopProbe]::Keys([uint32]$targetPid, $title, [bool]$inputAction.external, $keys)
} else {
    $edgeProcess = if ($inputAction.drop) { [uint32]$env:JOT_QA_EDGE } else { 0 }
    [DesktopProbe]::Drag([uint32]$env:JOT_QA_PID, $inputAction.title, $inputAction.x, $inputAction.y, $edgeProcess, $env:JOT_QA_HOST)
}
