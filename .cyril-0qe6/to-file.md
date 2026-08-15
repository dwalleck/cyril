# Tracker mutations queued for close-out (primary checkout, main)

- **cyril-tpwn note**: live-confirmed 2026-08-13 during the cyril-0qe6 live
  sweep — under an isolated HOME with real XDG_DATA_HOME, cyril's KAS bundle
  discovery fails ("run `kiro-cli acp --agent-engine v3` once to self-extract
  it, or set KIRO_KAS_SERVER_PATH") while a direct `kiro-cli acp
  --agent-engine kas` spawn in the same env works. cyril derives the bundle
  root from $HOME, not XDG_DATA_HOME. Workaround used in
  `.cyril-0qe6/live-sweep.py`: symlink `<fakehome>/.local/share` → real.
