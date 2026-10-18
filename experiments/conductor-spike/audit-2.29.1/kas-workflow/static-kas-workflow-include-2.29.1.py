#!/usr/bin/env python3
"""wf lane: the NEW `include` recipe node — schema, resolver, field-class map, gating."""
import re
from pathlib import Path
O=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js")
N=Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js")
old=O.read_text(encoding='utf-8',errors='replace'); new=N.read_text(encoding='utf-8',errors='replace')

print("### raw counts OLD->NEW")
for t in ('"include"',"'include'",'includedFrom','includeCount','includeDepth','MAX_INCLUDE','include paths cannot be resolved',
          'include_recipe','includeRecipe','Zho','legacyInclude','recipeInclude'):
    print(f"  {t:34} {old.count(t):4}->{new.count(t):4}")

print("\n### NEW: full include-node region (offsets 10,332,000..10,340,500)")
print(new[10332000:10340500])
