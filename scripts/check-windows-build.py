#!/usr/bin/env python3
"""Checks that a Windows build can start on every Windows 10/11 PC.

Usage: check-windows-build.py <folder or file>...

Looks at every native .exe and .dll given (folders are searched) for two
"works in CI, not on a normal PC" problems the CI runner can't show:

- A DLL that is neither in the build nor part of Windows 10 version 1809
  and later (import tables, normal and delay-loaded). GitHub's runners
  have extras installed, such as the Visual C++ runtime.
- A .NET app host .exe that isn't marked CET-compatible (.NET 9+ marks it
  unless CETCompat is false). The mark turns on Windows' hardware shadow
  stack, which stops return-address (ROP) attacks, on PCs with CET. Such
  a PC also needs a Windows update .NET relies on; the installer checks
  for it (windows/installer/Bookshelf.iss).

And that the .NET app host .exe carries the app's icon (ApplicationIcon),
which nothing else looks at.

Standard library only, so it runs on the CI runner and on Linux/macOS.
"""

import os
import struct
import sys

# DLLs that are part of Windows 10, version 1809 (the app's minimum) and later.
WINDOWS = {
    "advapi32.dll", "bcp47langs.dll", "bcp47mrm.dll", "bcrypt.dll",
    # combase: COM's home since Windows 8 (ole32's CoTaskMemFree forwards
    # to it). The Rust core links it for the known-folder lookup that finds
    # %LOCALAPPDATA% (directories -> dirs-sys -> windows-sys).
    "bcryptprimitives.dll", "combase.dll", "comctl32.dll", "coremessaging.dll",
    "crypt32.dll", "d2d1.dll", "d3d11.dll", "d3d12.dll", "d3dcompiler_47.dll",
    "dbghelp.dll", "dcomp.dll", "dwmapi.dll", "dwrite.dll", "dxgi.dll",
    "elscore.dll", "gdi32.dll", "imm32.dll", "iphlpapi.dll", "kernel32.dll",
    "mscms.dll", "ninput.dll", "ntdll.dll", "ole32.dll", "oleaut32.dll",
    "profapi.dll", "propsys.dll", "rometadata.dll", "rpcrt4.dll",
    "secur32.dll", "setupapi.dll", "shcore.dll", "shell32.dll",
    "shlwapi.dll", "sspicli.dll", "uiautomationcore.dll", "urlmon.dll",
    "user32.dll", "userenv.dll", "usp10.dll", "uxtheme.dll", "version.dll",
    "windowscodecs.dll", "winmm.dll", "ws2_32.dll", "xmllite.dll",
}

# Newer-Windows DLLs, allowed only for the Windows App SDK parts that the
# app never loads (Windows AI and search). If anything else starts to need
# them, the check fails so we can look at Windows 10 support again.
ONLY_FOR = {
    "icu.dll": {"microsoft.windows.search.dll"},  # Windows 10 1903+
    "dxcore.dll": {"npudetect.dll"},  # Windows 10 2004+
}


# Only .NET's app hosts (apphost, singlefilehost) contain this setting name.
DOTNET_HOST = "DOTNET_DISABLE_GUI_ERRORS".encode("utf-16-le")


def inspect(path):
    """Returns (is_managed, [imported DLL names], is_dotnet_host, is_cet,
    is_dotnet_host_without_icon) for a PE file, or None if it isn't one."""
    with open(path, "rb") as f:
        data = f.read()
    if data[:2] != b"MZ":
        return None
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        return None
    sections = struct.unpack_from("<H", data, pe + 6)[0]
    opt = pe + 24
    opt_size = struct.unpack_from("<H", data, pe + 20)[0]
    magic = struct.unpack_from("<H", data, opt)[0]
    dirs = opt + (112 if magic == 0x20B else 96)
    count = struct.unpack_from("<I", data, dirs - 4)[0]

    def directory(i):
        return struct.unpack_from("<II", data, dirs + 8 * i) if i < count else (0, 0)

    table = opt + opt_size
    spans = [struct.unpack_from("<IIII", data, table + 40 * i + 8) for i in range(sections)]

    def offset(rva):
        for size, va, raw_size, raw in spans:
            if va <= rva < va + max(size, raw_size):
                return raw + rva - va
        raise ValueError(f"RVA {rva:#x} is outside every section")

    def name(rva):
        start = offset(rva)
        return data[start:data.index(b"\0", start)].decode("ascii").lower()

    # .NET assemblies (IL or ReadyToRun) are loaded by the runtime, not by
    # Windows, so their stub import of mscoree.dll is never used.
    if directory(14)[0]:
        return True, [], False, False, False
    host = DOTNET_HOST in data

    # Extended DLL characteristics live in a debug directory entry
    # (type 20); bit 0 is IMAGE_DLLCHARACTERISTICS_EX_CET_COMPAT.
    cet = False
    rva, size = directory(6)  # debug directory: 28-byte entries
    for entry in range(offset(rva), offset(rva) + size, 28) if rva and host else ():
        kind = struct.unpack_from("<I", data, entry + 12)[0]
        raw = struct.unpack_from("<I", data, entry + 24)[0]
        if kind == 20 and struct.unpack_from("<I", data, raw)[0] & 1:
            cet = True

    # The resource directory's top level is one entry per resource type;
    # 14 is RT_GROUP_ICON. Named entries (high bit set) come first.
    no_icon = False
    if host:
        rva, _ = directory(2)
        types = []
        if rva:
            root = offset(rva)
            named, numbered = struct.unpack_from("<HH", data, root + 12)
            types = [struct.unpack_from("<I", data, root + 16 + 8 * i)[0] for i in range(named + numbered)]
        no_icon = 14 not in types

    names = []
    rva, _ = directory(1)  # import table: 20-byte entries, name at +12
    while rva:
        entry = offset(rva)
        if not any(data[entry:entry + 20]):
            break
        names.append(name(struct.unpack_from("<I", data, entry + 12)[0]))
        rva += 20
    image_base = struct.unpack_from("<Q", data, opt + 24)[0] if magic == 0x20B else \
        struct.unpack_from("<I", data, opt + 28)[0]
    rva, _ = directory(13)  # delay-load table: 32-byte entries, name at +4
    while rva:
        entry = offset(rva)
        if not any(data[entry:entry + 32]):
            break
        attributes, dll = struct.unpack_from("<II", data, entry)
        # Old-style entries (attribute bit 0 clear) hold addresses, not RVAs.
        names.append(name(dll if attributes & 1 else dll - image_base))
        rva += 32
    return False, names, host, cet, no_icon


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    paths = []
    for arg in sys.argv[1:]:
        if os.path.isdir(arg):
            paths += [os.path.join(d, f) for d, _, files in os.walk(arg) for f in files]
        elif os.path.isfile(arg):
            paths.append(arg)
        else:
            sys.exit(f"{arg} doesn't exist")
    shipped = {os.path.basename(p).lower() for p in paths}
    binaries = [p for p in paths if p.lower().endswith((".dll", ".exe"))]

    problems = []
    checked = hosts = 0
    for path in sorted(binaries):
        result = inspect(path)
        if result is None or result[0]:
            continue
        checked += 1
        importer = os.path.basename(path).lower()
        if result[2]:
            hosts += 1
            if not result[3]:
                problems.append(f"{os.path.basename(path)} isn't marked CET-compatible (remove CETCompat=false)")
        if result[4]:
            problems.append(f"{os.path.basename(path)} has no icon (set ApplicationIcon)")
        for dll in result[1]:
            if dll in shipped or dll in WINDOWS:
                continue
            # API sets are resolved by Windows itself (the C runtime's
            # api-ms-win-crt-* sets are part of Windows 10 too).
            if dll.startswith(("api-ms-win-", "ext-ms-win-")):
                continue
            if importer in ONLY_FOR.get(dll, ()):
                continue
            problems.append(f"{os.path.basename(path)} needs {dll}")

    if checked == 0:
        sys.exit("No native files found")
    if hosts == 0:
        sys.exit("No .NET app host (Bookshelf.exe) found")
    if problems:
        print("The app may not start on some Windows 10/11 PCs:")
        for problem in problems:
            print("  " + problem)
        sys.exit(1)
    print(f"OK: the {checked} native files only need each other and Windows,")
    print("and the .NET app host is marked CET-compatible and has its icon.")


if __name__ == "__main__":
    main()
