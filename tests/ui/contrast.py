#!/usr/bin/env python3
"""Audit script for theme color contrast in clients/web/assets/css/main.css.

Asserts WCAG 2.2 AA contrast ratios for both dark and light theme tokens:
- Normal body text and functional icons: contrast ratio >= 4.5:1
- Strong borders, control indicators, and UI components: contrast ratio >= 3.0:1
"""

import os
import re
import sys
from pathlib import Path

def srgb_to_linear(c: float) -> float:
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

def luminance(hex_str: str) -> float:
    hex_str = hex_str.strip().lstrip('#')
    if len(hex_str) == 3:
        hex_str = ''.join(c * 2 for c in hex_str)
    r, g, b = (int(hex_str[i:i+2], 16) for i in (0, 2, 4))
    return 0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b)

def contrast_ratio(c1: str, c2: str) -> float:
    l1 = luminance(c1)
    l2 = luminance(c2)
    return (max(l1, l2) + 0.05) / (min(l1, l2) + 0.05)

def parse_theme_tokens(css_content: str):
    # Remove @media blocks first so print and other media overrides don't overwrite base tokens
    cleaned = re.sub(r'@media[^{]+\{(?:[^{}]*\{[^{}]*\})*[^{}]*\}', '', css_content)

    blocks = {}
    pattern = re.compile(r'([.:]\w[\w-]*)\s*\{([^}]+)\}', re.MULTILINE)
    for match in pattern.finditer(cleaned):
        selector = match.group(1).strip()
        body = match.group(2)
        tokens = {}
        for line in body.split(';'):
            line = line.strip()
            if not line or ':' not in line:
                continue
            prop, val = line.split(':', 1)
            prop = prop.strip()
            val = val.strip()
            if prop.startswith('--'):
                hex_match = re.search(r'#[0-9a-fA-F]{3,8}', val)
                if hex_match:
                    tokens[prop] = hex_match.group(0)
                else:
                    tokens[prop] = val
        if selector in (':root', '.dark', '.light'):
            blocks[selector] = tokens

    dark_tokens = blocks.get('.dark', blocks.get(':root', {}))
    light_tokens = blocks.get('.light', {})
    return dark_tokens, light_tokens

def main():
    root = Path(__file__).resolve().parent.parent.parent
    css_path = root / 'clients/web/assets/css/main.css'

    if not css_path.exists():
        print(f"FAIL: CSS file not found at {css_path}")
        sys.exit(1)

    css_content = css_path.read_text(encoding='utf-8')
    dark, light = parse_theme_tokens(css_content)

    print("== Checking AutoMemory theme contrast ratios (WCAG 2.2 AA)")

    failures = []

    def check(theme_name: str, name: str, fg_hex: str, bg_hex: str, min_ratio: float, desc: str):
        ratio = contrast_ratio(fg_hex, bg_hex)
        passed = ratio >= min_ratio
        status = "PASS" if passed else "FAIL"
        print(f"  {status}  [{theme_name}] {desc} ({fg_hex} on {bg_hex}): {ratio:.2f}:1 (min {min_ratio:.1f}:1)")
        if not passed:
            failures.append(f"[{theme_name}] {desc} failed: {ratio:.2f}:1 < {min_ratio:.1f}:1")

    # Light Theme Asserts
    print("\n-- Light theme tests:")
    check("Light", "border-strong", light["--border-strong"], light["--bg"], 3.0, "strong border on bg")
    check("Light", "text", light["--text"], light["--bg"], 4.5, "body text on bg")
    check("Light", "text", light["--text"], light["--surface"], 4.5, "body text on surface (cards)")
    check("Light", "text-muted", light["--text-muted"], light["--surface"], 4.5, "muted text on surface (cards)")
    check("Light", "text-muted", light["--text-muted"], light["--surface-2"], 4.5, "muted text on surface-2")
    check("Light", "accent", light["--accent"], light["--surface"], 4.5, "accent text/links on cards")
    check("Light", "accent", light["--accent"], light["--bg"], 4.5, "accent text/links on bg")
    check("Light", "accent-solid", light["--accent-solid"], light["--accent-fg"], 4.5, "accent button fg on solid fill")
    check("Light", "ok", light["--ok"], light["--surface"], 4.5, "status ok on surface")
    check("Light", "warn", light["--warn"], light["--surface"], 4.5, "status warn on surface")
    check("Light", "bad", light["--bad"], light["--surface"], 4.5, "status bad on surface")
    check("Light", "info", light["--info"], light["--surface"], 4.5, "status info on surface")

    # Dark Theme Asserts
    print("\n-- Dark theme tests:")
    check("Dark", "text", dark["--text"], dark["--bg"], 4.5, "body text on bg")
    check("Dark", "text", dark["--text"], dark["--surface"], 4.5, "body text on surface (cards)")
    check("Dark", "text-muted", dark["--text-muted"], dark["--surface"], 4.5, "muted text on surface (cards)")
    check("Dark", "text-muted", dark["--text-muted"], dark["--surface-2"], 4.5, "muted text on surface-2")
    check("Dark", "accent", dark["--accent"], dark["--surface"], 4.5, "accent text on cards")
    check("Dark", "accent-solid", dark["--accent-solid"], dark["--accent-fg"], 4.5, "accent button fg on solid fill")
    check("Dark", "ok", dark["--ok"], dark["--surface"], 4.5, "status ok on surface")
    check("Dark", "warn", dark["--warn"], dark["--surface"], 4.5, "status warn on surface")
    check("Dark", "bad", dark["--bad"], dark["--surface"], 4.5, "status bad on surface")
    check("Dark", "info", dark["--info"], dark["--surface"], 4.5, "status info on surface")

    print()
    if failures:
        print(f"FAILED: {len(failures)} contrast assertions failed:")
        for f in failures:
            print(f"  - {f}")
        sys.exit(1)

    print("ALL CONTRAST ASSERTIONS PASSED (WCAG 2.2 AA compliant)")

if __name__ == '__main__':
    main()
