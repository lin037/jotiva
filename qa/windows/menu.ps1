# Inspect and click the real Win32 popup belonging to the disposable application.
$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
public static class MenuProbe {
    struct Point { public int X, Y; }
    struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32", CharSet=CharSet.Unicode)] static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string type, string title);
    [DllImport("user32")] static extern uint GetWindowThreadProcessId(IntPtr win, out uint pid);
    [DllImport("user32")] static extern IntPtr SendMessage(IntPtr win, uint message, IntPtr wParam, IntPtr lParam);
    [DllImport("user32")] static extern int GetMenuItemCount(IntPtr menu);
    [DllImport("user32", CharSet=CharSet.Unicode)] static extern int GetMenuString(IntPtr menu, uint index, StringBuilder text, int size, uint flags);
    [DllImport("user32")] static extern bool GetMenuItemRect(IntPtr win, IntPtr menu, uint index, out Rect rect);
    [DllImport("user32")] static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32")] static extern bool GetCursorPos(out Point point);
    [DllImport("user32")] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32")] static extern void mouse_event(uint flag, uint x, uint y, uint data, UIntPtr extra);
    static void Select(uint process, string label, bool click) {
        for (int attempt = 0; attempt < 35; ++attempt) {
            IntPtr popup = IntPtr.Zero;
            while ((popup = FindWindowEx(IntPtr.Zero,popup,"#32768",null)) != IntPtr.Zero) {
                uint pid;
                GetWindowThreadProcessId(popup, out pid);
                if (pid != process) continue;
                var menu = SendMessage(popup,0x1e1,IntPtr.Zero,IntPtr.Zero); // MN_GETHMENU
                for (uint i = 0; i < GetMenuItemCount(menu); ++i) {
                    var text = new StringBuilder(128);
                    GetMenuString(menu,i,text,text.Capacity,0x400); // MF_BYPOSITION
                    if (!text.ToString().Replace("&","").StartsWith(label)) continue;
                    Rect rect;
                    if (!GetMenuItemRect(IntPtr.Zero,menu,i,out rect)) continue;
                    var at = new Point {X=(rect.Left+rect.Right)/2,Y=(rect.Top+rect.Bottom)/2};
                    if (WindowFromPoint(at) != popup) continue;
                    SetCursorPos(at.X,at.Y);
                    if (click) {
                        mouse_event(2,0,0,0,UIntPtr.Zero);
                        Thread.Sleep(60);
                        mouse_event(4,0,0,0,UIntPtr.Zero);
                    }
                    return;
                }
            }
            Thread.Sleep(80);
        }
        throw new InvalidOperationException("Native menu item unavailable: "+label);
    }
    public static void OpenSettings(uint process) {
        SetThreadDpiAwarenessContext((IntPtr)(-4));
        Point previous;
        GetCursorPos(out previous);
        try {
            Select(process,"Tools",false);
            Select(process,"Settings",true);
        } finally { SetCursorPos(previous.X,previous.Y); }
    }
}
"@
[MenuProbe]::OpenSettings([uint32]$env:JOT_QA_PID)
