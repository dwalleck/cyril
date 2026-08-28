#!/usr/bin/env python3
"""
Extract the KAS feature-flag registry (flag -> default -> env var) from a KAS
bundle. Re-run each release to regenerate the table in the wire audit.

    python3 extract-kas-feature-flags.py [<acp-server.js>]

Defaults to the newest bundle under ~/.local/share/kiro-cli/kas/.

Three objects hold the registry. In a MINIFIED bundle (KAS >= 0.54.3) their
variable names are minifier-assigned (`Fo`, `$O`, `UAi` in 0.54.3) and WILL be
reassigned in later builds, so this script never anchors on them — it anchors on
the string literals, which minification cannot rename:

  flags     CONST:"wire_key"          anchored on a known wire key
  defaults  wire_key:{default:…}      anchored on the same key
  env map   [X.CONST]:"KIRO_FEATURE_…_ENABLED"   matched directly

Flags absent from the env map are EXPERIMENT-ONLY: backend-resolved, reachable
neither by a client nor by spawn-time env. The env provider is boolean-only
("true"/"false", trimmed); anything else logs featureConfig.env.unparsable and
falls through to the default, so a typo silently yields default behaviour.

Note: only works on bundles that carry this registry (introduced with the
stream-idle watchdog in KAS 0.54.3 / kiro-cli 2.20.1). Earlier unminified
bundles predate it and exit with a clear message.
"""
import glob, os, re, sys

ANCHOR = "stream_idle_watchdog"   # a wire key that lives in both objects


def newest_bundle():
    root = os.path.expanduser("~/.local/share/kiro-cli/kas")
    dirs = [d for d in glob.glob(os.path.join(root, "*-*"))
            if os.path.isdir(d) and not d.endswith(".lock")]
    if not dirs:
        raise SystemExit(f"no KAS bundle under {root}")
    def ver(d):
        m = re.match(r"(\d+)\.(\d+)\.(\d+)", os.path.basename(d))
        return tuple(int(x) for x in m.groups()) if m else (0, 0, 0)
    return os.path.join(max(dirs, key=ver),
                        "node_modules/@kiro/agent/dist/server/acp-server.js")


def enclosing_object(src, needle_re, label):
    """Brace-match the object literal that CONTAINS the first match of needle_re."""
    m = re.search(needle_re, src)
    if not m:
        raise SystemExit(f"[{label}] anchor not found — bundle shape changed: {needle_re}")
    # walk backwards to the '{' that opens this object
    depth, i = 0, m.start()
    while i >= 0:
        if src[i] == "}":
            depth += 1
        elif src[i] == "{":
            if depth == 0:
                break
            depth -= 1
        i -= 1
    else:
        raise SystemExit(f"[{label}] no enclosing '{{' before anchor")
    # walk forwards to its match
    depth, j = 0, i
    while j < len(src):
        if src[j] == "{":
            depth += 1
        elif src[j] == "}":
            depth -= 1
            if depth == 0:
                return src[i:j + 1]
        j += 1
    raise SystemExit(f"[{label}] unterminated object literal")


path = sys.argv[1] if len(sys.argv) > 1 else newest_bundle()
src = open(path, encoding="utf-8", errors="replace").read()

if f'"{ANCHOR}"' not in src:
    raise SystemExit(f"{path}\n  no '{ANCHOR}' — bundle predates the feature-config "
                     f"registry (KAS < 0.54.3). Nothing to extract.")

flags = dict(re.findall(r'(\w+):"([^"]+)"',
                        enclosing_object(src, rf'\w+:"{ANCHOR}"', "flags")))
defaults = dict(re.findall(r'(\w+):\{default:(![01]|"[a-z]+")\}',
                           enclosing_object(src, rf'{ANCHOR}:\{{default:', "defaults")))
# env map: matched directly, no object anchor needed
envs = dict(re.findall(r'\[\w+\.(\w+)\]:"(KIRO_FEATURE_[A-Z0-9_]+)"', src))


def render(v):
    return {"!0": "true", "!1": "false"}.get(v, v.strip('"') if v else "?")


print(f"bundle: {path}\n")
print("| wire key | default | env var |")
print("|---|---|---|")
for const, key in flags.items():
    print(f"| `{key}` | `{render(defaults.get(key))}` | "
          f"{'`' + envs[const] + '`' if const in envs else '— none —'} |")

missing = [flags[c] for c in flags if c not in envs]
print(f"\n{len(flags)} flags; {len(envs)} env-reachable; {len(missing)} experiment-only")
print("experiment-only:", ", ".join(missing))

undefaulted = [k for k in flags.values() if k not in defaults]
if undefaulted:
    print("WARNING: flags with no default entry:", ", ".join(undefaulted))
orphan_env = [c for c in envs if c not in flags]
if orphan_env:
    print("WARNING: env vars for unknown flag consts:", ", ".join(orphan_env))
