#!/usr/bin/env python3
"""
Generate version_info.txt for PyInstaller on Windows.
Embeds VS_VERSION_INFO metadata into the PE executable to eliminate
unbranded executable heuristics and false-positive antivirus detections.
"""
import sys
import re

def parse_version(v_str: str):
    m = re.findall(r"\d+", v_str)
    nums = [int(x) for x in m[:4]]
    while len(nums) < 4:
        nums.append(0)
    return tuple(nums)

def main():
    version_str = sys.argv[1] if len(sys.argv) > 1 else "1.0.0"
    out_path = sys.argv[2] if len(sys.argv) > 2 else "version_info.txt"
    v_tuple = parse_version(version_str)
    v_dotted = ".".join(str(x) for x in v_tuple)

    content = f"""# UTF-8
VSVersionInfo(
  ffi=FixedFileInfo(
    filevers={v_tuple},
    prodvers={v_tuple},
    mask=0x3f,
    flags=0x0,
    OS=0x40004,
    fileType=0x1,
    subtype=0x0,
    date=(0, 0)
  ),
  kids=[
    StringFileInfo(
      [
        StringTable(
          '040904b0',
          [
            StringStruct('CompanyName', 'SQL Editor Project'),
            StringStruct('FileDescription', 'SQL Editor - Lightweight Database Client'),
            StringStruct('FileVersion', '{v_dotted}'),
            StringStruct('InternalName', 'sql-editor'),
            StringStruct('LegalCopyright', 'Copyright (c) 2026 SQL Editor Contributors'),
            StringStruct('OriginalFilename', 'sql-editor.exe'),
            StringStruct('ProductName', 'SQL Editor'),
            StringStruct('ProductVersion', '{v_dotted}')
          ]
        )
      ]
    ),
    VarFileInfo([VarStruct('Translation', [1033, 1200])])
  ]
)
"""
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(content)
    print(f"Generated {out_path} for version {v_dotted}")

if __name__ == "__main__":
    main()

