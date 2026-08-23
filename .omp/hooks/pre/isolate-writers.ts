import type { HookAPI, HookContext } from "@oh-my-pi/pi-coding-agent/extensibility/hooks";
// Agent names are not a trust boundary: project or user definitions can
// override bundled read-only agents, so every non-plan subagent gets its own
// workspace. Plan mode already restricts children to read-only tools and rejects
// per-spawn isolation.

function isolateTask(item: unknown): unknown {
  if (item === null || typeof item !== "object" || Array.isArray(item)) {
    return item;
  }

  return { ...item, isolated: true };
}

function isPlanMode(ctx: HookContext): boolean {
  const branch = ctx.sessionManager.getBranch();
  for (let index = branch.length - 1; index >= 0; index -= 1) {
    const entry = branch[index];
    if (entry?.type === "mode_change") {
      return entry.mode === "plan";
    }
  }
  return false;
}

export default function isolateSubagents(pi: HookAPI): void {
  pi.on("tool_call", async (event, ctx) => {
    if (event.toolName !== "task" || isPlanMode(ctx)) {
      return;
    }

    const input = event.input as Record<string, unknown>;
    if (Array.isArray(input.tasks)) {
      return {
        input: {
          ...input,
          tasks: input.tasks.map(isolateTask),
        },
      };
    }

    return { input: { ...input, isolated: true } };
  });
}
