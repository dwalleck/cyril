# Windows handoff: /voice over ACP on kiro-cli 2.18.0

**Goal:** answer the one open question from cyril's voice research — what does the
dark-launched `/voice` ACP command actually DO on a build that has the voice
engine? Linux always returns `{"success":false,"message":"Voice mode is not
supported on this platform"}`, so no one has ever seen a successful `/voice`
session over ACP. Windows builds compile the full engine (onnxruntime/Whisper,
`voice.rs`/`voice_serve.rs` — verified statically on the 2.16.0 MSI), and 2.18.0
lifted the advertise gate, so this is now testable with zero special flags.

**Cost: zero credits.** The probe never sends `session/prompt`; voice commands are
local. **Privacy:** transcription is local Whisper — no audio leaves the machine.

## 1. Prerequisites (once)

1. Windows 10/11 with a working microphone.
   Settings → Privacy & security → Microphone → allow desktop apps.
2. Install kiro-cli **2.18.0** (MSI `kiro-cli-x86_64-pc-windows-msvc.msi` from the
   release origin, or your normal install path) and log in: `kiro-cli login`.
   Verify: `kiro-cli --version` → `kiro-cli 2.18.0`.
3. Python 3: `winget install Python.Python.3.12` (or any existing `py -3`).
4. Copy this folder (`windows-voice-probe-2.18.0/`) to the Windows box.

## 2. Recommended: separate the download from the wire test

The Whisper model downloads once, on first use, behind a confirmation. Do it
OUTSIDE the wire probe so Phase C measures recording, not downloading — either:

- **TUI route:** run `kiro-cli`, press Ctrl+O (or type `/voice`), accept the
  download prompt, say a few words, Enter. If this works, the engine is healthy —
  everything after this isolates the *ACP surface* specifically.
- **CLI route:** `kiro-cli voice --confirm-download`, speak, press Enter.
  (`--confirm-download` skips the interactive prompt — the TUI uses exactly this
  flag after its own confirm dialog.)

If NEITHER works, stop and report that — the engine itself is broken on this
machine and the ACP probe would only measure that failure.

## 3. Run the probe

```powershell
cd <this folder>
py -3 probe_v2_voice_win.py
# or, if kiro-cli is not on PATH:
py -3 probe_v2_voice_win.py "C:\Program Files\Kiro CLI\kiro-cli.exe"
```

**You must SPEAK when the console says `>>> SPEAK NOW <<<`** (~25 s window, twice).
Say something distinctive: *"cyril wire probe testing one two three."*

The script: checks `kiro-cli voice --help` parses natively (Linux says
`unrecognized subcommand` — Windows should NOT), then over ACP: initialize →
session/new → confirms `/voice` in `commands/available` → `voice status` →
`voice start` (waits up to 5 min in case the model downloads anyway) → records
all wire traffic while you speak → `voice status` mid-recording → `voice stop`
→ a second start/stop cycle. Every frame is captured; every server→client
request is auto-answered (permissions: allow) and printed loudly.

## 4. What we expect to see (recovered event shapes)

There are no TypeScript definitions in tui.js (types are erased at bundling),
but the runtime handlers are the de facto schema. The TUI's two voice transports
(recovered from the 2.18.0 bundle) speak these events:

**Local helper** (`kiro-cli voice [--ptt] [--confirm-download]`, JSON lines on
stdout; stop = newline on stdin; cancel = SIGKILL):

```json
{"type":"level","value":0.42}          // mic level
{"type":"status","value":"..."}        // lifecycle status
{"type":"partial","value":"testing"}   // streaming partial transcript
{"type":"needs_download","value":...}  // model missing -> confirm flow
{"type":"error","value":{"message":"..."}}
{"type":"text","value":"final transcript"}
```

**Remote SSE** (`POST {url}/voice/record/stream`, `data:` lines; stop =
`POST /voice/record/stop`):

```json
{"type":"activity","level":0.42}
{"type":"done","text":"final transcript"}
{"type":"error","message":"..."}
```

The Rust ACP `/voice` command wraps the same engine, so if anything streams over
ACP, expect THIS vocabulary (partial/level/status/text/needs_download) — either
inside `commands/execute` responses or as some notification. If instead
`voice stop`'s response carries only a final transcript (or nothing at all and
the flow expects a client-side injection like the TUI does), that is equally
decisive: it tells cyril whether ACP voice is a *stream* or a *request/response*
surface, or advertised-but-inert on the wire.

## 5. What to bring back

Three files (same folder after the run):

- `v2-voice-win-2.18.0.jsonl` — full wire capture
- `v2-voice-win-2.18.0.jsonl.stderr` — agent stderr
- console output (copy-paste or `py -3 probe_v2_voice_win.py *> console.txt`)

Drop them into `experiments/conductor-spike/windows-voice-probe-2.18.0/` on the
Linux checkout and tell the next Claude session:
*"Windows voice probe results are in — fold into the voice memory."* It will
update `reference_kiro_voice_subsystem` (open probe from item 6) and
`reference_kiro_2_18_0_diff`, and decide whether cyril needs code (e.g. voice
notification rendering) or not.

## 6. Interpretation cheat sheet

| Observation | Meaning for cyril |
|---|---|
| `voice --help` fails to parse | Windows build ships without engine after all — re-verify binary/version, report |
| `/voice` absent from commands/available | Advertise gate is platform-conditional — memory correction needed |
| start returns the Linux-style refusal | Engine gated off over ACP even on Windows — `/voice` is TUI-only everywhere; cyril can ignore it |
| start ok, silence on wire, stop returns transcript | ACP voice = request/response; cyril could add `/voice` UX with a simple response handler |
| notifications stream during recording | ACP voice = live stream; capture tells us method + shape; cyril needs a renderer decision |
| `needs_download` anywhere on the wire | Download confirm rides ACP — cyril must surface it |
| server→client request during start | Confirm/permission flow over ACP — shape is in the capture |
