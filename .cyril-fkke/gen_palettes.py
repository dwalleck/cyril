#!/usr/bin/env python3
"""Generate the Rust `source()` palette dispatcher from the oracle tables.

One-off authoring aid: keeps the Rust literals byte-identical to the tables the
independent oracle checks. Run:

  python3 .cyril-fkke/gen_palettes.py > .cyril-fkke/generated_source.rs
"""

from __future__ import annotations

import importlib.util
import pathlib

HERE = pathlib.Path(__file__).parent
spec = importlib.util.spec_from_file_location("oracle", HERE / "palette-oracle.py")
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)

SYNTAX = {
    "CyrilDark": "Base16EightiesDark",
    "CyrilLight": "Base16OceanLight",
    "HighContrastDark": "Base16OceanDark",
    "HighContrastLight": "InspiredGitHub",
    "CatppuccinMocha": "Base16MochaDark",
    "GruvboxDark": "Base16EightiesDark",
}

DOC = {
    "CyrilDark": "The fixed Cyril Dark palette: fixed brightened RGB held to per-tier WCAG targets (ADR 0007, tests::cyril_dark_contrast_contract).",
    "CyrilLight": "Cyril Light: dark foregrounds on a painted light canvas (tier policy: .cyril-fkke/design.md).",
    "HighContrastDark": "High Contrast Dark: AAA targets (>=7.0 primary, >=4.5 muted) on a painted black canvas.",
    "HighContrastLight": "High Contrast Light: AAA targets on a painted white canvas.",
    "CatppuccinMocha": "Catppuccin Mocha: upstream Mocha colors adjusted only where a tier required it; Syntect has no Catppuccin component, so base16-mocha.dark is the nearest available syntax theme.",
    "GruvboxDark": "Gruvbox Dark (medium): upstream Gruvbox colors adjusted only where a tier required it; base16-eighties.dark is the nearest bundled syntax theme.",
}

out = []
out.append("fn source(id: ThemeId) -> SourceTheme {\n")
out.append("    match id {\n")
for name, (palette, _backgrounds, _tier) in oracle.PALETTES.items():
    out.append(f"        // {DOC[name]}\n")
    out.append(f"        ThemeId::{name} => SourceTheme {{\n")
    out.append(f"            syntax: SyntaxTheme::{SYNTAX[name]},\n")
    for role, value in palette.items():
        if value is None:
            out.append(f"            {role}: SourceColor::Reset,\n")
        else:
            r, g, b = oracle.rgb(value)
            out.append(f"            {role}: SourceColor::Rgb(0x{r:02x}, 0x{g:02x}, 0x{b:02x}),\n")
    out.append("        },\n")
out.append("    }\n")
out.append("}\n")
print("".join(out), end="")
