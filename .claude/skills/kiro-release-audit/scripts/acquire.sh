#!/usr/bin/env bash
# acquire.sh <ver> [arch] — download, verify, extract one kiro-cli release into the research archive.
# Idempotent: existing tarball/extraction are reused. Writes only under $ARCHIVE/binaries/<ver>/.
set -euo pipefail
ver=${1:?usage: acquire.sh <ver> [x86_64|aarch64]}
arch=${2:-x86_64}
ARCHIVE=${ARCHIVE:-$HOME/.local/share/kiro-research}
ORIGIN=https://desktop-release.q.us-east-1.amazonaws.com
MANIFEST=https://prod.download.cli.kiro.dev/stable/latest/manifest.json
dir="${ARCHIVE:?}/binaries/${ver:?}"
mkdir -p "$dir"

tar=""
for ext in tar.xz tar.zst; do
  f="$dir/kirocli-$arch-linux.$ext"
  if [ -s "$f" ]; then tar=$f; break; fi
  url="$ORIGIN/$ver/kirocli-$arch-linux.$ext"
  code=$(curl -sI -o /dev/null -w '%{http_code}' --max-time 30 "$url")
  if [ "$code" = 200 ]; then
    echo "downloading $url"
    curl -sS --retry 3 --max-time 1200 -o "$f" "$url"
    tar=$f; break
  fi
done
[ -n "$tar" ] || { echo "no tarball on origin for $ver ($arch)"; exit 1; }

sha=$(sha256sum "$tar" | cut -d' ' -f1)
echo "$sha" > "$tar.sha256"
echo "sha256 $sha"
if curl -s --max-time 20 -o "$dir/manifest.latest.json" "$MANIFEST"; then
  python3 -I - "$dir/manifest.latest.json" "$ver" "$(basename "$tar")" "$sha" <<'EOF'
import json, sys
m = json.load(open(sys.argv[1])); ver, fname, sha = sys.argv[2:5]
cur = m.get("currentVersion") or m.get("version")
want = None
target = f"{ver}/{fname}"  # the manifest lists tar.xz, tar.zst, tar.gz, zip, rpm and -musl variants per arch: match the exact file
def walk(o):
    global want
    if isinstance(o, dict):
        if str(o.get("download", "")) == target: want = o
        for v in o.values(): walk(v)
    elif isinstance(o, list):
        for v in o: walk(v)
walk(m)
if cur != ver: print(f"manifest is for {cur}, not {ver}: sha recorded, not verified")
elif want and want.get("sha256") == sha: print("MATCHES latest manifest")
else: print("MISMATCH vs latest manifest:", want and want.get("sha256"))
EOF
fi

ex="$dir/extracted"
if [ ! -d "$ex/kirocli/bin" ]; then
  mkdir -p "$ex"
  case "$tar" in
    *.tar.xz) tar -xJf "$tar" -C "$ex" ;;
    *.tar.zst) tar --zstd -xf "$tar" -C "$ex" ;;
  esac
fi
cp "$ex/kirocli/BUILD-INFO" "$dir/BUILD-INFO"
cat "$dir/BUILD-INFO"; echo
ls -la "$ex/kirocli/bin/"
inst=$(command -v kiro-cli-chat 2>/dev/null || true)
if [ -n "$inst" ] && cmp -s "$ex/kirocli/bin/kiro-cli-chat" "$inst"; then
  echo "installed kiro-cli-chat == archived $ver"
fi
echo "ACQUIRE-OK $ver"
