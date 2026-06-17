# KAS spec-workflow sample output (2.7.1)

The full requirements → design → tasks arc, produced live by
`probe-kas-spec-design-2.7.1.py` from the prompt:
> "Create a spec for a small CLI tool `csv2json` that reads a CSV file and writes a
> JSON array of row objects, with a --pretty flag."

Each phase is a separate async `_kiro/spec/invoke` call (returns `{sessionId}`, then
streams a full agent turn) that self-scaffolds `.kiro/specs/<feature>/` on disk:

| File | Produced by | Shape |
|---|---|---|
| `config.kiro.json` | `createSpec` | `.config.kiro` metadata: `{specId, workflowType:"requirements-first", specType:"feature"}` |
| `requirements.md` | `invoke {operation:'createSpec'}` | EARS acceptance criteria (`WHEN/IF/WHILE … THE … SHALL`) + Glossary + User Stories |
| `design.md` | `invoke {operation:'generateDocument', documentType:'design'}` | Overview, Architecture (mermaid), Components/Interfaces, Data Models, Correctness Properties, Error Handling, Testing Strategy |
| `tasks.md` | `invoke {operation:'generateDocument', documentType:'tasks'}` | Checkbox implementation plan; sub-tasks; `_Requirements: X.Y_` traceability links; optional property-test tasks |

`_kiro/spec/getTaskStatuses {tasksFilePath, featureName, workspacePaths}` then returns
the hierarchical task tree (`{taskId, markdownStatus, isLeaf, isOptional, subTasks[]}`)
parsed from `tasks.md`'s checkboxes.

These files are model-generated samples, committed to illustrate the on-disk spec
layout and document formats — they are not part of cyril's build.
