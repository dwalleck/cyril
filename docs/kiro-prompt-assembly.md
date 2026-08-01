# Kiro prompt assembly — what the model actually receives

> **Evidence class: backend-side capture.** Derived from AWS **prompt logs**
> (the Amazon Q Developer administrative feature that ships
> `GenerateAssistantResponse` payloads to S3). Sample: 328 files / 435 records
> from a single day, 2026-07-23, against kiro-cli builds current at that date.
>
> **The corpus is not in this repository.** It came from a private workspace and
> contains account identifiers, user identifiers, absolute home paths, and
> proprietary source excerpts. Every example below is structural — real values
> are replaced with neutral placeholders. Do not commit prompt-log captures.

## Why this vantage point is new

Every other wire artifact in this repo — `KIRO_ACP_RECORD_PATH` traces, the
`experiments/kiro-proxy-rs/` logging proxy, the `conductor-spike` captures —
observes the **ACP wire**. Prompt assembly happens *downstream* of that, inside
kiro-cli, after cyril's `session/prompt` has been received.

Prompt logs sit on the far side of assembly. They are the only artifact that
shows what Kiro **built** out of cyril's input before handing it to the model.

Two limits worth stating up front:

- `prompt` is empty on 362 of 435 records. Those are tool-result turns, where the
  current message carries `toolResults` rather than `content`. Only 73 records
  carry assembled text.
- The log captures `userInputMessage.content` only. **No system prompt, no tool
  schemas, no conversation history.** This shows the injection layer, not the
  whole context window.

Record shape:

```jsonc
{"records": [{
  "generateAssistantResponseEventRequest":  {"prompt": "…", "chatTriggerType": "MANUAL",
                                             "userId": "…", "timeStamp": "…", "modelId": "…"},
  "generateAssistantResponseEventResponse": {"assistantResponse": "…",
                                             "codeReferenceEvents": [], "requestId": "…"}
}]}
```

## Three assembly shapes

### 1. Rules + scaffold (33 records)

```text
## Included Rules (<rule-id>) [Global]

  [System-injected rules — apply these constraints to your work, but continue
   answering the user's request. Do not acknowledge or summarize these rules
   in your response.]

<user-rule id=<rule-id>>
```
…verbatim file body…
```
</user-rule>

## Included Rules (AGENTS.md) [Workspace]

  [System-injected rules — …]
  Workspace-level rules take precedence over global-level rules when conflicts exist.

<user-rule id=AGENTS.md>…</user-rule>

<progress_reporting>…use the reportProgress tool every 4-5 turns…</progress_reporting>
<response_requirement>…you MUST use the subagentResponse tool…</response_requirement>
<current_context>Machine ID: acp-client</current_context>
<session_start_snapshot captured="session-start">
This file tree was captured at session start. Treat it as accurate unless …
<file_tree><fileTree>
<folder name='<abs-path>'>
  <file name='<abs-path>' />
  <folder name='<abs-path>' closed />
</folder>
</fileTree></file_tree>
</session_start_snapshot>

<the caller's task text>
```

### 2. Delimiter shape (25 records)

```text
--- CONTEXT ENTRY BEGIN ---
Current time: <weekday>, <ISO-8601 with offset>
--- CONTEXT ENTRY END ---

--- USER MESSAGE BEGIN ---
<the user's text>
--- USER MESSAGE END ---
```

A second `CONTEXT ENTRY` payload carries the todo list, on turns with no user
message at all:

```text
--- CONTEXT ENTRY BEGIN ---
Active Task List for current session:

Description: <goal>
Progress: 0/4 tasks completed

Tasks:
[ ] #1. <task> (NEXT)
…
--- CONTEXT ENTRY END ---
```

### 3. Attachment shape (11 records)

```text
<session_context_files>
The user attached these files to the session as additional context. Treat them
as current; re-read with read_file if a later action suggests they may have changed.
…file bodies…
The current model is <model display name>.
</session_context_files>

<the user's text>
```

Note the model's own display name is injected here — the model is told which
model it is.

### Delegation wrappers

```text
<orchestrator_briefing>…workspace path, repo layout, prior steps…</orchestrator_briefing>
<user_instruction>…the actual task…</user_instruction>
<how_to_interpret>
The text inside <user_instruction> is the user's task. It is the authoritative
requirement: every constraint, file path, exact string, numeric value, and
example must be honored exactly as written.
…
If the briefing and the user instruction conflict on what to deliver, the user
instruction wins. …
</how_to_interpret>
```

## Load-bearing facts for cyril

1. **Caller text always lands LAST.** All three shapes terminate with the text
   the client supplied. Every injection precedes it. cyril cannot place content
   *after* its own prompt through any legitimate channel — but anything it puts
   in a steering file gets both the "system rules" framing and earlier position.

2. **Precedence is stated in-band.** The rules header literally tells the model
   *"Workspace-level rules take precedence over global-level rules"* and *"Do not
   acknowledge or summarize these rules"*. Rule precedence is a prompt
   convention, not an engine guarantee.

3. **Steering rules propagate into subagents.** All 33 subagent-scaffold records
   carried the full `## Included Rules` stack. A steering file reaches every
   child session; a `_session/steer` does not.

4. **`<user-rule>` bodies are fenced but not escaped.** The body is wrapped in a
   bare triple-backtick fence, and 20 of 80 observed bodies contain *inner*
   triple-backtick fences (an `AGENTS.md` with a mermaid block and a directory
   map). Only the closing `</user-rule>` tag disambiguates the end.
   **Unverified hypothesis:** a steering file containing a fence followed by
   `</user-rule>` may escape into top-level scaffolding. Probe before relying on
   or worrying about this — schema-shaped reasoning has been wrong here before.

5. **Injection dominates the turn.** Across the 73 content-bearing turns:
   ~800k characters assembled against ~99k typed — **87% invisible scaffolding**.
   The `session_start_snapshot` file tree alone was a fixed ~8.6k characters
   (~2.1k tokens), 45% of the turn it appeared in. Caveat: this is per-turn
   assembled content and says nothing about prompt caching or history reuse.

## The steer receipt (implemented — cyril-3qwa)

`_session/steer` does not reach the model as bare text. It arrives as its own
user turn with a mandatory reply contract:

```text
[LIVE STEERING - New message from user]

The user sent a new message while you are working. As the currently active
agent, adjust your approach if necessary based on this guidance.

<user_message id="steer-<uuid>">
<the steer text>
</user_message>

IMPORTANT: After completing your work, include a brief note about how you
handled this steering message. Use this exact format:

[STEERING steer-<uuid>: <describe what you did or why it wasn't applicable>]
```

The model complies — the captured `assistantResponse` ends with a well-formed
`[STEERING steer-<uuid>: …]` trailer.

The id shares the wire queue-id space: the committed KAS capture
`session_info_update_steering_queued.json` carries `messageId` in the same
`steer-<uuid>` shape, which is what makes correlation back to a `SteerEcho`
possible.

cyril harvests this trailer in `cyril-core/src/types/steer_receipt.rs`. Two
wire facts it depends on, both verified by
`experiments/conductor-spike/probe-steering-receipt-chunking-2.16.0.py`
(mock backend, zero credits):

- the trailer rides `session/update: agent_message_chunk` and nothing else;
- split across chunks it is **never whole in any single notification**, which is
  why the harvester withholds a partial-marker tail from the live view.

## Open questions

- **ACP-driven or subagent-driven?** All 33 records carrying
  `Machine ID: acp-client` are *also* subagent turns, so "ACP client gets the
  rules + snapshot stack" and "subagents get it" are not separable from this
  sample. It matters: if the snapshot is ACP-driven, every cyril session pays
  ~2.1k tokens for a file tree it never requested and cannot display. Resolving
  it needs a one-axis probe — one plain main-agent turn over ACP, one over the
  native TUI, same day, same binary. See the wire-audit methodology note on
  isolating binary-vs-backend axes.
- **Is the `<user-rule>` fence escapable?** See fact 4.
- **Does `--- CONTEXT ENTRY ---` accept client-supplied entries**, or is it
  strictly engine-generated? No sample shows more than one entry per turn.
