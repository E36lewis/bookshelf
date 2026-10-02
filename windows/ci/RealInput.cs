// Keys and clicks for the UI tour, sent the way a keyboard and a mouse
// send them: through SendInput, keys by their scan codes (Windows turns
// those into virtual keys with the keyboard layout, as for a real
// keyboard), modifiers held down a moment before the key and let go after
// it, as people press them. Loaded by ui-tour.ps1 with Add-Type, so it's
// C# 5 (Windows PowerShell's compiler).
//
// Send() takes the SendKeys notation the tour was written in: ^ Ctrl,
// + Shift, % Alt for the next key; {ENTER}, {F11}, {DOWN 12} (12 times);
// any other character is typed with the keys that make it on this layout.

using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Threading;

public static class RealInput
{
    const uint INPUT_MOUSE = 0;
    const uint INPUT_KEYBOARD = 1;
    const uint KEYEVENTF_EXTENDEDKEY = 0x1;
    const uint KEYEVENTF_KEYUP = 0x2;
    const uint KEYEVENTF_UNICODE = 0x4;
    const uint KEYEVENTF_SCANCODE = 0x8;
    const uint MOUSEEVENTF_LEFTDOWN = 0x2;
    const uint MOUSEEVENTF_LEFTUP = 0x4;
    const uint MAPVK_VK_TO_VSC_EX = 4;

    public const ushort Shift = 0x10;
    public const ushort Control = 0x11;
    public const ushort Alt = 0x12;

    [StructLayout(LayoutKind.Sequential)]
    struct MOUSEINPUT { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

    [StructLayout(LayoutKind.Sequential)]
    struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

    [StructLayout(LayoutKind.Explicit)]
    struct InputUnion { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; }

    [StructLayout(LayoutKind.Sequential)]
    struct INPUT { public uint type; public InputUnion u; }

    [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint count, INPUT[] inputs, int size);
    [DllImport("user32.dll")] static extern uint MapVirtualKey(uint code, uint mapType);
    [DllImport("user32.dll")] static extern short VkKeyScan(char c);
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);

    static readonly Dictionary<string, ushort> Named = new Dictionary<string, ushort>
    {
        { "ENTER", 0x0D }, { "ESC", 0x1B }, { "TAB", 0x09 }, { "BACKSPACE", 0x08 }, { "BS", 0x08 },
        { "DELETE", 0x2E }, { "DEL", 0x2E }, { "HOME", 0x24 }, { "END", 0x23 }, { "PGUP", 0x21 }, { "PGDN", 0x22 },
        { "LEFT", 0x25 }, { "UP", 0x26 }, { "RIGHT", 0x27 }, { "DOWN", 0x28 }, { "SPACE", 0x20 },
        { "F1", 0x70 }, { "F2", 0x71 }, { "F3", 0x72 }, { "F4", 0x73 }, { "F5", 0x74 }, { "F6", 0x75 },
        { "F7", 0x76 }, { "F8", 0x77 }, { "F9", 0x78 }, { "F10", 0x79 }, { "F11", 0x7A }, { "F12", 0x7B },
    };

    /// <summary>Sends keys written as for SendKeys ("^b", "{F11}", "^{END}{ENTER}Text").</summary>
    public static void Send(string keys)
    {
        var modifiers = new List<ushort>();
        for (var i = 0; i < keys.Length; i++)
        {
            var c = keys[i];
            if (c == '^') { modifiers.Add(Control); continue; }
            if (c == '+') { modifiers.Add(Shift); continue; }
            if (c == '%') { modifiers.Add(Alt); continue; }
            if (c == '{')
            {
                var close = keys.IndexOf('}', i + 2);
                if (close < 0) throw new ArgumentException("No closing } in " + keys);
                var parts = keys.Substring(i + 1, close - i - 1).Split(' ');
                i = close;
                var times = parts.Length > 1 ? int.Parse(parts[1]) : 1;
                ushort vk;
                if (Named.TryGetValue(parts[0].ToUpperInvariant(), out vk)) Press(modifiers.ToArray(), vk, times);
                else if (parts[0].Length == 1) for (var n = 0; n < times; n++) TypeChar(modifiers, parts[0][0]);
                else throw new ArgumentException("Unknown key {" + parts[0] + "}");
                modifiers.Clear();
                continue;
            }
            TypeChar(modifiers, c);
            modifiers.Clear();
        }
    }

    /// <summary>
    /// Holds the modifiers, presses the key <paramref name="times"/> times,
    /// and lets the modifiers go, with a person's pauses in between.
    /// </summary>
    public static void Press(ushort[] modifiers, ushort vk, int times)
    {
        var held = new List<ushort>();
        try
        {
            foreach (var m in modifiers)
            {
                Key(m, false);
                held.Add(m);
                Thread.Sleep(35);
            }
            var hold = modifiers.Length > 0 ? 45 : 10;
            for (var n = 0; n < times; n++)
            {
                Key(vk, false);
                Thread.Sleep(hold);
                Key(vk, true);
                Thread.Sleep(modifiers.Length > 0 ? 25 : 10);
            }
        }
        finally
        {
            for (var i = held.Count - 1; i >= 0; i--)
            {
                Key(held[i], true);
                Thread.Sleep(15);
            }
        }
    }

    /// <summary>
    /// Holds the modifiers down for <paramref name="holdMs"/> before the key,
    /// repeating the last one as a held key does (after half a second, 30
    /// times a second), as someone does who finds Ctrl first and then B.
    /// </summary>
    public static void PressHeld(ushort[] modifiers, ushort vk, int holdMs)
    {
        var held = new List<ushort>();
        try
        {
            foreach (var m in modifiers)
            {
                Key(m, false);
                held.Add(m);
                Thread.Sleep(35);
            }
            for (var waited = 0; waited < holdMs; waited += 33)
            {
                Thread.Sleep(33);
                if (waited >= 500) Key(modifiers[modifiers.Length - 1], false); // the key repeating
            }
            Key(vk, false);
            Thread.Sleep(60);
            Key(vk, true);
            Thread.Sleep(40);
        }
        finally
        {
            for (var i = held.Count - 1; i >= 0; i--)
            {
                Key(held[i], true);
                Thread.Sleep(15);
            }
        }
    }

    /// <summary>A left click at a point on the screen.</summary>
    public static void Click(int x, int y)
    {
        if (!SetCursorPos(x, y)) throw new Win32Exception(Marshal.GetLastWin32Error());
        Thread.Sleep(80);
        Mouse(MOUSEEVENTF_LEFTDOWN);
        Thread.Sleep(70);
        Mouse(MOUSEEVENTF_LEFTUP);
        Thread.Sleep(80);
    }

    /// <summary>Moves the pointer out of the way (so no tooltip shows in a screenshot).</summary>
    public static void MoveTo(int x, int y)
    {
        SetCursorPos(x, y);
        Thread.Sleep(50);
    }

    static void TypeChar(List<ushort> modifiers, char c)
    {
        var scan = VkKeyScan(c);
        if (scan == -1)
        {
            // Not on this keyboard: as a character, the way an input method sends one.
            Unicode(c, false);
            Unicode(c, true);
            Thread.Sleep(10);
            return;
        }
        var all = new List<ushort>(modifiers);
        if ((scan & 0x100) != 0 && !all.Contains(Shift)) all.Add(Shift);
        if ((scan & 0x200) != 0 && !all.Contains(Control)) all.Add(Control);
        if ((scan & 0x400) != 0 && !all.Contains(Alt)) all.Add(Alt);
        Press(all.ToArray(), (ushort)(scan & 0xFF), 1);
    }

    static void Key(ushort vk, bool up)
    {
        var input = new INPUT { type = INPUT_KEYBOARD };
        var scan = MapVirtualKey(vk, MAPVK_VK_TO_VSC_EX);
        // The keys between the letters and the number pad (arrows, Home,
        // Delete...) share scan codes with the number pad's and are told
        // apart by the extended flag (MapVirtualKey doesn't always say so).
        if (vk >= 0x21 && vk <= 0x2E && scan != 0) scan |= 0xE000;
        if (scan == 0)
        {
            input.u.ki.wVk = vk;
            input.u.ki.dwFlags = up ? KEYEVENTF_KEYUP : 0;
        }
        else
        {
            input.u.ki.wScan = (ushort)(scan & 0xFF);
            input.u.ki.dwFlags = KEYEVENTF_SCANCODE
                | ((scan & 0xFF00) != 0 ? KEYEVENTF_EXTENDEDKEY : 0)
                | (up ? KEYEVENTF_KEYUP : 0);
        }
        Send(input);
    }

    static void Unicode(char c, bool up)
    {
        var input = new INPUT { type = INPUT_KEYBOARD };
        input.u.ki.wScan = c;
        input.u.ki.dwFlags = KEYEVENTF_UNICODE | (up ? KEYEVENTF_KEYUP : 0);
        Send(input);
    }

    static void Mouse(uint flags)
    {
        var input = new INPUT { type = INPUT_MOUSE };
        input.u.mi.dwFlags = flags;
        Send(input);
    }

    static void Send(INPUT input)
    {
        if (SendInput(1, new[] { input }, Marshal.SizeOf(typeof(INPUT))) != 1)
        {
            throw new Win32Exception(Marshal.GetLastWin32Error(), "SendInput was refused");
        }
    }
}
