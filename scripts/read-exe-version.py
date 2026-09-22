#!/usr/bin/env python3
"""读取 PE 的 VERSIONINFO（实测项 T5 的观测工具，见 docs/实测-T1-T8.md）。

T5 要回答的是「`VERSIONINFO` 是否确为多段构建号、段数是否稳定」，以及它能不能
当作游戏版本的识别来源。**终末地的实测给出了反面答案**：`Endfield.exe` 的
`VERSIONINFO` 是 Unity 引擎版本（`2021.3.34.0`），形态完全合法但语义错误 ——
这类「看起来合理的假值」没有任何环节会报错，因此必须靠预先观测排除。

用法：
    python scripts/read-exe-version.py                     # 读内置的默认路径
    python scripts/read-exe-version.py <exe 或目录> [...]   # 目录会被展开为其中的 *.exe

输出：`VS_FIXEDFILEINFO` 的 FileVersion / ProductVersion，以及 `StringFileInfo`
的四个常见键。两者并不总是相同，而实现要挑一个作为识别来源，所以都要看。
"""
import ctypes
import sys
from ctypes import wintypes
from pathlib import Path

# 本机已知路径（T5 的默认观测对象）。加新游戏时补在这里。
DEFAULT_TARGETS = [
    Path(r"D:\新建文件夹\Hypergryph Launcher\games\Endfield Game\Endfield.exe"),
    Path(r"D:\新建文件夹\Hypergryph Launcher\games\Endfield Game\PlatformProcess.exe"),
    Path(r"D:\新建文件夹\Hypergryph Launcher\Launcher.exe"),
]

version = ctypes.windll.version
version.GetFileVersionInfoSizeW.argtypes = [wintypes.LPCWSTR, ctypes.POINTER(wintypes.DWORD)]
version.GetFileVersionInfoSizeW.restype = wintypes.DWORD
version.GetFileVersionInfoW.argtypes = [
    wintypes.LPCWSTR,
    wintypes.DWORD,
    wintypes.DWORD,
    ctypes.c_void_p,
]
version.GetFileVersionInfoW.restype = wintypes.BOOL
version.VerQueryValueW.argtypes = [
    ctypes.c_void_p,
    wintypes.LPCWSTR,
    ctypes.POINTER(ctypes.c_void_p),
    ctypes.POINTER(wintypes.UINT),
]
version.VerQueryValueW.restype = wintypes.BOOL

STRING_KEYS = ("FileVersion", "ProductVersion", "FileDescription", "ProductName")


class VS_FIXEDFILEINFO(ctypes.Structure):
    _fields_ = [
        ("dwSignature", wintypes.DWORD),
        ("dwStrucVersion", wintypes.DWORD),
        ("dwFileVersionMS", wintypes.DWORD),
        ("dwFileVersionLS", wintypes.DWORD),
        ("dwProductVersionMS", wintypes.DWORD),
        ("dwProductVersionLS", wintypes.DWORD),
        ("dwFileFlagsMask", wintypes.DWORD),
        ("dwFileFlags", wintypes.DWORD),
        ("dwFileOS", wintypes.DWORD),
        ("dwFileType", wintypes.DWORD),
        ("dwFileSubtype", wintypes.DWORD),
        ("dwFileDateMS", wintypes.DWORD),
        ("dwFileDateLS", wintypes.DWORD),
    ]


def _quad(ms: int, ls: int) -> str:
    """两个 DWORD → `a.b.c.d`（Windows 把 4 段版本装进两个 DWORD）。"""
    return f"{ms >> 16}.{ms & 0xFFFF}.{ls >> 16}.{ls & 0xFFFF}"


def _query(buf, sub_block: str):
    ptr = ctypes.c_void_p()
    length = wintypes.UINT()
    if not version.VerQueryValueW(buf, sub_block, ctypes.byref(ptr), ctypes.byref(length)):
        return None
    return ctypes.wstring_at(ptr, length.value).rstrip("\x00")


def read(path: Path) -> dict:
    text = str(path)
    size = version.GetFileVersionInfoSizeW(text, None)
    if not size:
        return {"error": "没有 VERSIONINFO 资源（这在 .dll/.exe 上都可能出现）"}
    buf = ctypes.create_string_buffer(size)
    if not version.GetFileVersionInfoW(text, 0, size, buf):
        return {"error": "GetFileVersionInfo 失败"}

    ptr = ctypes.c_void_p()
    length = wintypes.UINT()
    version.VerQueryValueW(buf, "\\", ctypes.byref(ptr), ctypes.byref(length))
    ffi = ctypes.cast(ptr, ctypes.POINTER(VS_FIXEDFILEINFO)).contents

    # 语言/代码页必须枚举，不能假设 0409 —— 国内发行物常见 0804（简体中文）
    trans_ptr = ctypes.c_void_p()
    trans_len = wintypes.UINT()
    version.VerQueryValueW(
        buf,
        "\\VarFileInfo\\Translation",
        ctypes.byref(trans_ptr),
        ctypes.byref(trans_len),
    )
    strings: dict[str, str] = {}
    for i in range(trans_len.value // 4):
        lang, codepage = ctypes.cast(
            trans_ptr.value + i * 4, ctypes.POINTER(wintypes.WORD * 2)
        ).contents
        for key in STRING_KEYS:
            value = _query(buf, f"\\StringFileInfo\\{lang:04x}{codepage:04x}\\{key}")
            if value and key not in strings:
                strings[key] = value

    return {
        "fixed_file": _quad(ffi.dwFileVersionMS, ffi.dwFileVersionLS),
        "fixed_product": _quad(ffi.dwProductVersionMS, ffi.dwProductVersionLS),
        "strings": strings,
    }


def expand(targets: list[Path]) -> list[Path]:
    """目录 → 其中的 *.exe（不递归到 AntiCheatExpert 之类子目录，避免噪声）。"""
    out: list[Path] = []
    for target in targets:
        if target.is_dir():
            out.extend(sorted(target.glob("*.exe")))
        else:
            out.append(target)
    return out


def main() -> int:
    targets = [Path(a) for a in sys.argv[1:]] or DEFAULT_TARGETS
    missing = 0
    for path in expand(targets):
        print(f"=== {path.name} ===")
        print(f"    path: {path}")
        if not path.exists():
            print("    [跳过] 文件不存在\n")
            missing += 1
            continue
        info = read(path)
        if "error" in info:
            print(f"    {info['error']}\n")
            continue
        print(f"    VS_FIXEDFILEINFO.FileVersion    : {info['fixed_file']}")
        print(f"    VS_FIXEDFILEINFO.ProductVersion : {info['fixed_product']}")
        for key in STRING_KEYS:
            if key in info["strings"]:
                print(f"    StringFileInfo.{key:<16}: {info['strings'][key]}")
        print()
    return 0 if not missing else 1


if __name__ == "__main__":
    raise SystemExit(main())
