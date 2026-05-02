---
title: Pi
description: What the sidebar shows for Pi panes, and how the extension bridge maps its events.
---

Pi works with the sidebar through a TypeScript extension that emits the same
normalized hook events consumed by the rest of the app.

## Supported signals

| Sidebar surface        | Pi source event |
| ---------------------- | --------------- |
| Session start          | `session_start` |
| Running status + prompt | `before_agent_start` |
| Activity log           | `tool_execution_start` |
| Stop / idle status     | `agent_end` |
| Session cleanup        | `session_shutdown` |

## Limitations

| Feature                  | Current behavior |
| ------------------------ | ---------------- |
| Permission badge          | Pi does not expose Claude-style permission modes |
| Waiting status            | Not currently emitted by the bridge |
| Background shell state    | Pi has no built-in background bash mode |
| Sub-agent tree            | Pi core does not ship sub-agents by default |
| Worktree lifecycle tracking | Pi does not emit Claude-style `WorktreeCreate` / `WorktreeRemove` events |

## Setup

Wire the extension from [Pi setup](/tmux-agent-sidebar/getting-started/pi/).
