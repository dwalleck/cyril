# KAS spec-workflow sample output (2.7.1)

Produced live by `probe-kas-spec-2.7.1.py` from the prompt:
> "Create a spec for a small CLI tool `csv2json` that reads a CSV file and writes a
> JSON array of row objects, with a --pretty flag."

via `_kiro/spec/resolveSession {strategy:'fresh'}` → `_kiro/spec/invoke {operation:'createSpec', userPrompt}`.

- `config.kiro.json` — the on-disk `.kiro/specs/<feature>/.config.kiro` metadata
  (`{specId, workflowType:"requirements-first", specType:"feature"}`).
- `requirements.md` — the generated requirements doc: EARS-style acceptance criteria
  (WHEN/IF/WHILE … THE … SHALL), a Glossary, and User Stories. **`createSpec` produces
  the requirements phase only** — `design.md`/`tasks.md` come from subsequent
  `_kiro/spec/invoke {operation:'generateDocument', documentType:'design'|'tasks'}` calls.
