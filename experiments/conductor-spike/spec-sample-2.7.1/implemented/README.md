# executeTask output sample

`src-types.ts` is the real source `_kiro/spec/invoke {operation:'executeTask'}` wrote
for task **"1.2 Create core type definitions and interfaces"** (a leaf task from the
generated `tasks.md`), produced live by `probe-kas-spec-executetask-2.7.1.py`.

executeTask: async (returns `{sessionId, executionId}` — note it DOES carry an
executionId, unlike createSpec/generateDocument), delegates to the bundled
`spec-task-execution` subagent (which List Directory / File Search / Write File /
Read File), marks the task via a `task_status` tool, and flips the task in `tasks.md`
so `getTaskStatuses` reports `markdownStatus:"completed", executionStatus:"succeed"`.
