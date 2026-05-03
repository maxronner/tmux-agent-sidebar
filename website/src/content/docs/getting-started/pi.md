---
title: Pi setup
description: Wire up the Pi extension bridge from the bundled plugin directory.
---

Pi works with the sidebar through a local extension bridge. Once the extension
is visible to Pi, it emits sidebar hook events for session start, prompts, tool
activity, stop, and shutdown.

## Install the extension

Create Pi's global extension directory if it does not already exist, then
symlink the bundled extension into it:

```sh
mkdir -p ~/.pi/agent/extensions
ln -sf "${XDG_CONFIG_HOME:-$HOME/.config}/tmux/plugins/tmux-agent-sidebar/.pi/extensions/tmux-agent-sidebar.ts" \
  ~/.pi/agent/extensions/tmux-agent-sidebar.ts
```

If you installed the plugin somewhere else, replace the source path with your
plugin path. The bridge also checks `TMUX_AGENT_SIDEBAR_DIR`,
`${XDG_CONFIG_HOME:-$HOME/.config}/tmux/plugins/tmux-agent-sidebar`, and the
classic `~/.tmux/plugins/tmux-agent-sidebar` location when resolving the binary.

## Reload Pi

Inside Pi, run:

```text
/reload
```

Or restart Pi. New Pi panes inside tmux should appear in the sidebar as `pi`.
