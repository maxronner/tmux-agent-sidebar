import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const AGENT = "pi";

type CommandSpec = {
  cmd: string;
  prefix: string[];
};

function pluginDirs(): string[] {
  const home = process.env.HOME ?? "";
  const xdgConfig = process.env.XDG_CONFIG_HOME || (home ? `${home}/.config` : "");
  const dirs = [
    process.env.TMUX_AGENT_SIDEBAR_DIR ?? "",
    ...walkUpFromExtension(),
    xdgConfig ? `${xdgConfig}/tmux/plugins/tmux-agent-sidebar` : "",
    home ? `${home}/.tmux/plugins/tmux-agent-sidebar` : "",
  ];
  return [...new Set(dirs.filter(Boolean))];
}

function walkUpFromExtension(): string[] {
  const dirs: string[] = [];
  let dir = dirname(fileURLToPath(import.meta.url));
  for (let i = 0; i < 5; i += 1) {
    dirs.push(dir);
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return dirs;
}

function resolveCommand(): CommandSpec {
  for (const pluginDir of pluginDirs()) {
    const hookScript = resolve(pluginDir, "hook.sh");
    if (existsSync(hookScript)) {
      return { cmd: "bash", prefix: [hookScript, AGENT] };
    }
  }

  for (const pluginDir of pluginDirs()) {
    for (const bin of [
      resolve(pluginDir, "bin/tmux-agent-sidebar"),
      resolve(pluginDir, "target/release/tmux-agent-sidebar"),
    ]) {
      if (existsSync(bin)) {
        return { cmd: bin, prefix: ["hook", AGENT] };
      }
    }
  }

  return { cmd: "tmux-agent-sidebar", prefix: ["hook", AGENT] };
}

const command = resolveCommand();

function emit(event: string, payload: Record<string, unknown> = {}) {
  if (!process.env.TMUX_PANE) return;

  try {
    const child = spawn(command.cmd, [...command.prefix, event], {
      stdio: ["pipe", "ignore", "ignore"],
      env: process.env,
    });
    child.on("error", () => {});
    child.stdin.on("error", () => {});
    child.stdin.end(JSON.stringify(payload));
  } catch {
    // Pi must keep working even when the sidebar plugin/binary is missing.
  }
}

function sessionId(ctx: { sessionManager: { getSessionFile(): string | undefined } }) {
  return ctx.sessionManager.getSessionFile() ?? undefined;
}

function textFromMessage(message: any): string {
  const content = message?.content;
  if (!Array.isArray(content)) return "";
  return content
    .map((part) => (part?.type === "text" && typeof part.text === "string" ? part.text : ""))
    .filter(Boolean)
    .join("\n");
}

export default function (pi: ExtensionAPI) {
  pi.on("session_start", (event, ctx) => {
    emit("session-start", {
      cwd: ctx.cwd,
      source: event.reason,
      session_id: sessionId(ctx),
    });
  });

  pi.on("before_agent_start", (event, ctx) => {
    emit("user-prompt-submit", {
      cwd: ctx.cwd,
      prompt: event.prompt,
      session_id: sessionId(ctx),
    });
  });

  pi.on("tool_execution_start", (event, ctx) => {
    emit("activity-log", {
      cwd: ctx.cwd,
      session_id: sessionId(ctx),
      tool_name: event.toolName,
      tool_input: event.args ?? {},
      tool_response: {},
    });
  });


  pi.on("agent_end", (event, ctx) => {
    const lastAssistant = [...event.messages]
      .reverse()
      .find((message: any) => message?.role === "assistant");

    emit("stop", {
      cwd: ctx.cwd,
      session_id: sessionId(ctx),
      last_message: textFromMessage(lastAssistant),
    });
  });

  pi.on("session_shutdown", (event, _ctx) => {
    emit("session-end", {
      end_reason: event.reason,
    });
  });
}
