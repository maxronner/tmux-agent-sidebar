-- tmux-agent-sidebar bridge for smelt.
--
-- Subscribes to smelt reactive cells and emits normalized hook events to
-- the tmux-agent-sidebar binary (or hook.sh) so the sidebar can track
-- smelt sessions alongside Claude Code, Codex, OpenCode, and Pi.
--
-- Install by symlinking this file into smelt's plugin directory:
--
--   mkdir -p ~/.config/smelt/plugins
--   ln -sf ~/.tmux/plugins/tmux-agent-sidebar/.smelt/plugins/tmux-agent-sidebar.lua \
--     ~/.config/smelt/plugins/tmux-agent-sidebar.lua
--
-- Or if using a worktree-based install:
--
--   ln -sf <worktree>/.smelt/plugins/tmux-agent-sidebar.lua \
--     ~/.config/smelt/plugins/tmux-agent-sidebar.lua

local AGENT = "smelt"

-- ── resolve command ──────────────────────────────────────────────────

local function resolve_command()
	local home = os.getenv("HOME") or ""
	local xdg_config_home = os.getenv("XDG_CONFIG_HOME") or (home .. "/.config")
	local plugin_dirs = {}
	local configured_dir = os.getenv("TMUX_AGENT_SIDEBAR_DIR")
	if configured_dir and configured_dir ~= "" then
		plugin_dirs[#plugin_dirs + 1] = configured_dir
	end
	plugin_dirs[#plugin_dirs + 1] = xdg_config_home .. "/tmux/plugins/tmux-agent-sidebar"
	plugin_dirs[#plugin_dirs + 1] = home .. "/.tmux/plugins/tmux-agent-sidebar"

	for _, plugin_dir in ipairs(plugin_dirs) do
		local hook_script = plugin_dir .. "/hook.sh"
		local f = io.open(hook_script, "r")
		if f then
			f:close()
			return { cmd = "bash", prefix = { hook_script, AGENT } }
		end

		-- Fall back to a pre-built binary.
		for _, bin in ipairs({
			plugin_dir .. "/bin/tmux-agent-sidebar",
			plugin_dir .. "/target/release/tmux-agent-sidebar",
		}) do
			f = io.open(bin, "r")
			if f then
				f:close()
				return { cmd = bin, prefix = { "hook", AGENT } }
			end
		end
	end

	-- Last resort: hope it's on $PATH.
	return { cmd = "tmux-agent-sidebar", prefix = { "hook", AGENT } }
end

local command = resolve_command()

-- ── JSON encoder (simple, just for our payload shapes) ───────────────

local function json_escape(str)
	return str:gsub('[%z\1-\31\\"]', function(c)
		local escapes = {
			['"'] = '\\"',
			["\\"] = "\\\\",
			["\b"] = "\\b",
			["\f"] = "\\f",
			["\n"] = "\\n",
			["\r"] = "\\r",
			["\t"] = "\\t",
		}
		return escapes[c] or string.format("\\u%04x", string.byte(c))
	end)
end

local function json_encode(val)
	local t = type(val)
	if t == "string" then
		return '"' .. json_escape(val) .. '"'
	elseif t == "number" then
		return tostring(val)
	elseif t == "boolean" then
		return val and "true" or "false"
	elseif t == "nil" then
		return "null"
	elseif t == "table" then
		local parts = {}
		local is_array = true
		local max_key = 0
		for k in pairs(val) do
			if type(k) == "number" and k > max_key then
				max_key = k
			elseif type(k) ~= "number" then
				is_array = false
			end
		end
		if is_array and max_key > 0 then
			for i = 1, max_key do
				parts[i] = json_encode(val[i])
			end
			return "[" .. table.concat(parts, ",") .. "]"
		else
			local keys = {}
			for k in pairs(val) do
				keys[#keys + 1] = k
			end
			table.sort(keys)
			for _, k in ipairs(keys) do
				parts[#parts + 1] = json_encode(k) .. ":" .. json_encode(val[k])
			end
			return "{" .. table.concat(parts, ",") .. "}"
		end
	end
	return "null"
end

-- ── fire-and-forget hook emit ────────────────────────────────────────

local function shell_quote(str)
	return "'" .. tostring(str):gsub("'", "'\\''") .. "'"
end

local function command_line(event)
	local parts = { shell_quote(command.cmd) }
	for _, arg in ipairs(command.prefix) do
		parts[#parts + 1] = shell_quote(arg)
	end
	parts[#parts + 1] = shell_quote(event)
	return table.concat(parts, " ")
end

local function emit(event, payload)
	if not os.getenv("TMUX_PANE") then
		return
	end
	local json = json_encode(payload or {})
	local cmd = string.format("printf %%s %s | %s >/dev/null 2>&1 &", shell_quote(json), command_line(event))
	os.execute(cmd)
end

-- ── shared payload helpers ───────────────────────────────────────────

local function session_payload()
	return {
		cwd = smelt.session.cwd(),
		session_id = smelt.session.id(),
	}
end

-- ── cell subscriptions ───────────────────────────────────────────────

local function emit_session_start()
	emit("session-start", {
		cwd = smelt.session.cwd(),
		session_id = smelt.session.id(),
		source = "startup",
	})
end

-- Global plugins load after Smelt's startup `ready` event and initial
-- `session_started` publication. Register the current session immediately so a
-- newly opened idle session appears in the sidebar.
emit_session_start()

-- Later session changes (/new, /resume, /fork) reach cell subscribers.
smelt.cell("session_started"):subscribe(function()
	emit_session_start()
end)

-- Session ended: emit session-end.
smelt.cell("session_ended"):subscribe(function()
	emit("session-end", { end_reason = "quit" })
end)

-- Input submitted: emit user-prompt-submit with the prompt text.
smelt.cell("input_submit"):subscribe(function(text)
	local payload = session_payload()
	payload.prompt = text or ""
	emit("user-prompt-submit", payload)
end)

-- Turn ended: emit stop when a turn completes without cancellation.
smelt.cell("turn_end"):subscribe(function(payload)
	if payload and payload.cancelled then
		return
	end
	emit("stop", session_payload())
end)

-- Turn error: emit stop-failure.
smelt.cell("turn_error"):subscribe(function(payload)
	local err = ""
	if payload then
		if type(payload) == "string" then
			err = payload
		elseif payload.message then
			err = payload.message
		elseif payload.error then
			err = payload.error
		end
	end
	emit("stop-failure", {
		cwd = smelt.session.cwd(),
		session_id = smelt.session.id(),
		error = err,
	})
end)

-- Tool start: emit activity-log with tool name and input.
smelt.cell("tool_start"):subscribe(function(payload)
	if not payload then
		return
	end
	local tool_name = ""
	local tool_input = {}
	if type(payload) == "table" then
		tool_name = payload.tool or payload.name or payload.tool_name or ""
		tool_input = payload.args or payload.input or payload.tool_input or {}
	end
	if tool_name == "" then
		return
	end
	emit("activity-log", {
		cwd = smelt.session.cwd(),
		session_id = smelt.session.id(),
		tool_name = tool_name,
		tool_input = tool_input,
		tool_response = {},
	})
end)
