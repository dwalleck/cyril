#!/usr/bin/env python3
"""Independent lexical oracle for the seams exercised by probe-runtime.py."""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
CONFIG = (ROOT / "crates/cyril-core/src/types/config.rs").read_text(encoding="utf-8")
THEME = (ROOT / "crates/cyril-ui/src/theme.rs").read_text(encoding="utf-8")
STATE = (ROOT / "crates/cyril-ui/src/state.rs").read_text(encoding="utf-8")
APP = (ROOT / "crates/cyril/src/app.rs").read_text(encoding="utf-8")


def enum_variants(source: str, name: str) -> list[str]:
    body = re.search(rf"pub enum {name}\s*\{{(.*?)\n\}}", source, re.S)
    if body is None:
        raise SystemExit(f"missing enum {name}")
    return re.findall(r"^\s{4}([A-Za-z][A-Za-z0-9_]*)\s*,", body.group(1), re.M)


ui = re.search(r"pub struct UiConfig\s*\{(.*?)\n\}", CONFIG, re.S)
if ui is None:
    raise SystemExit("missing UiConfig")
fields = re.findall(r"^\s{4}pub ([a-z_]+):", ui.group(1), re.M)
picker = STATE[STATE.index("// --- Picker dialog methods ---"):STATE.index("// --- Hooks panel methods ---")]
confirm = re.search(r"pub fn picker_confirm.*?\n    \}", picker, re.S)
cancel = re.search(r"pub fn picker_cancel.*?\n    \}", picker, re.S)
if confirm is None or cancel is None:
    raise SystemExit("missing picker lifecycle")
app_picker = APP[APP.index("async fn handle_picker_key"):APP.index("async fn submit_input")]

output = [
    f"QAQ0_ORACLE config_fields={','.join(fields)}",
    f"QAQ0_ORACLE config_has_theme={'theme' in fields} config_has_color_mode={'color_mode' in fields}",
    f"QAQ0_ORACLE theme_ids={','.join(enum_variants(THEME, 'ThemeId'))}",
    f"QAQ0_ORACLE color_modes={','.join(enum_variants(THEME, 'ColorMode'))}",
    f"QAQ0_ORACLE automatic_present={'Automatic' in enum_variants(THEME, 'ColorMode')}",
    f"QAQ0_ORACLE picker_confirm_value_only={'Option<(String, String)>' in confirm.group(0)}",
    f"QAQ0_ORACLE picker_cancel_discards={'self.picker = None' in cancel.group(0)}",
    f"QAQ0_ORACLE picker_mutates_theme={'self.theme' in picker}",
    f"QAQ0_ORACLE picker_confirm_dispatches_agent={'BridgeCommand::ExecuteCommand' in app_picker}",
]
sys.stdout.write("\n".join(output) + "\n")
