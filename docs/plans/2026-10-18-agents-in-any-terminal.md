# Agents typed in a terminal: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `claude` or `codex` a person types at the prompt of a Aegis terminal (bash, zsh, fish, PowerShell) gets
Aegis's MCP server, hooks and status line, as the ones Aegis opens do (kanban card 42).

**Architecture:** The words `typed()` appends after an agent's command are also built as raw arguments, and every new
terminal carries them in `AEGIS_CLAUDE_ARGS` and `AEGIS_CODEX_ARGS` (one word per line, empty when there is
nothing to give). The shell integration scripts define a `claude` and a `codex` function that put those words before
the person's. One private helper holds the rules (data folder, program, WSL, connections, cmd and Windows PowerShell),
so what is typed and what the variables carry never differ.

**Tech Stack:** Rust 2024; bash (3.2 up: macOS), zsh, fish, PowerShell 7 and 5.1; the real-shell PTY tests of
`aegis-terminal/src/integration.rs` (`mod shells`, `Home`); headless GPUI tests (`workspace/tests/`).

**Spec:** [docs/specs/connections.md](../specs/connections.md#agents-typed-in-a-terminal) (read that section first:
the rules, what is left out and why), and [shell-integration.md](../specs/shell-integration.md).

## Global Constraints

- Variables: `AEGIS_CLAUDE_ARGS`, `AEGIS_CODEX_ARGS`. Values are raw words joined by `\n`, no shell quoting;
  Windows' `\` written `/`. **Both are set on every new terminal**, empty when there is nothing to give: an empty value
  stops one inherited from a Aegis this one was started from (as `AEGIS_BUS` does).
- Claude Code's words: `--settings`, `<data>/claude-statusline.json`, then `--mcp-config=<data>/mcp.json` (with `=`,
  one word) only when `typed` would give `--mcp-config`. Codex's: `-c` and each value of `codex_flags`, unquoted.
- The function puts the words **before** the person's: `command claude <words> "$@"`.
- It adds nothing when the person's words hold `--settings`, `--settings=…`, `--mcp-config` or `--mcp-config=…`
  (Claude Code), or a word starting with `mcp_servers.aegis.` (Codex); nor when its variable is empty at the call.
- Defined only when its variable is non-empty as the shell starts and there is no function of that name (zsh: defined
  in `.zshenv` after the user's `.zshenv`, so the `.zshrc`'s replaces it; PowerShell: no alias either). Use the
  `function` keyword in bash and zsh (an alias of the same name would break `claude() {`).
- bash code runs on bash 3.2: no `mapfile`, no `local -n`.
- Nothing on PATH, no file outside Aegis's data folder, no new dependency, no `unsafe`; only `crates/aegis` uses
  gpui-kit.
- Copy: the switch's label **Agents in Aegis**; its description "Claude Code and Codex opened from the canvas or
  typed in a terminal get Aegis's server and hooks: they read and type into the blocks wired to them";
  `config.toml`'s comment "The Claude Code and Codex Aegis opens, or that are typed in its terminals, get its MCP
  server and hooks: they can read and type into the blocks wired to them (docs/specs/connections.md)."
- Each commit passes `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`,
  on `development`, titled as CLAUDE.md says. A task that changes a spec's behavior updates its lines in the same commit.
  No bench: nothing here paints or types into agents.

## Review Focus

1. **A path with a space in Aegis's words** (macOS's `~/Library/Application Support/aegis`): one argument in
   every shell. Tasks 1, 2 and 3 put one in their words.
2. **A Aegis started from a Aegis terminal** (`cargo run` in one): its terminals' variables are empty, never the
   first one's. Task 1 pins it.
3. **An alias named `claude` in the `.bashrc`**, read before the integration defines its function: bash still parses
   the script, and the alias's words come after Aegis's. Task 2 pins it.
4. **The agent's exit status** reaches the shell (`claude; echo $?`, the prompt mark's color). Tasks 2 and 3 pin it
   with a fake that exits 3.
5. **`claude` with no argument**, the usual way: Aegis's words only; and `claude ''` keeps the empty argument.
   Tasks 2 and 3 pin it.

---

### Task 1: The words as arguments, on every new terminal

**Files:**
- Modify: `crates/aegis-mcp/src/lib.rs` (`codex_args`; `codex_flags` built from it)
- Modify: `crates/aegis-usage/src/claude.rs` (`settings_file`, `mcp_file`; `settings_arg`, `mcp_arg` built on them)
- Modify: `crates/aegis/src/workspace/usage.rs` (`claude_args`, `Gives`, `gives`, `agent_env`; `typed` uses `gives`)
- Modify: `crates/aegis/src/workspace/blocks.rs` (`spawn_block`: both env arms get `self.agent_env()`)
- Test: `crates/aegis-mcp/src/lib.rs`, `crates/aegis-usage/src/claude.rs`, `crates/aegis/src/workspace/tests/usage.rs`,
  `crates/aegis/src/workspace/tests/bus.rs`

**Interfaces:**
- Produces:
  - `pub fn aegis_mcp::codex_args(exe: &str, windows: bool, hooks: bool) -> Option<Vec<String>>`: `-c`, value,
    `-c`, value…, the values `codex_flags` single-quotes; same `None` rule. `codex_flags` becomes these words, each
    value as ` -c '<value>'`.
  - `pub fn aegis_usage::claude::settings_file(data: &Path, exe: &Path, windows: bool, status_line: bool, hooks: bool) -> Option<PathBuf>`
    and `pub fn mcp_file(data: &Path, exe: &Path, windows: bool) -> Option<PathBuf>`: what `settings_arg` and
    `mcp_arg` do today (write the file when it changed; `None` under the same conditions, a path `typed_path` refuses
    included), returning the file's path. `settings_arg`/`mcp_arg` = `typed_path` of it.
  - `pub(super) fn claude_args(data: &Path, exe: &Path, windows: bool, limits: bool, connected: bool) -> Vec<String>`
    in `workspace/usage.rs`, beside `claude_flags`, same conditions.
  - `struct Gives { data: PathBuf, exe: PathBuf, connected: bool, codex: bool, on_path: bool }` and
    `fn gives(&self) -> Option<Gives>` (private to `usage.rs`): `None` without a data folder or a program, or under WSL;
    `connected` = `[connections] enabled` and a bus; `codex` = `connected` and the shell is neither cmd nor
    powershell; `on_path` as `typed` computes it. `typed` keeps its output, now through `gives`.
  - `pub(super) fn agent_env(&self) -> Vec<(String, String)>` on `Workspace`: always both variables, in this order:
    `("AEGIS_CLAUDE_ARGS", claude_args(..).join("\n"))`, `("AEGIS_CODEX_ARGS", codex_args(..).join("\n"))`
    (`""` when `gives` is `None`, or `!codex`, or `codex_args` is `None`).

- [ ] **Step 1: Write the failing tests**
  - `aegis-mcp` `codex_args_are_the_flags_unquoted`: `codex_args("/opt/Aegis App/aegis", false, true)` has 20
    words, every even one `-c`; word 1 is `mcp_servers.aegis.command="/opt/Aegis App/aegis"`; the last is
    `hooks.Interrupt=[{hooks=[{type="command",command="aegis hook codex",timeout=3}]}]`; with `hooks` false, 8
    words; `/home/o'neil/aegis` gives `None`. The existing `codex_gets_the_server_and_its_hooks_in_single_quotes`
    stays as it is.
  - `aegis-usage` `settings_file_is_the_path_unquoted`: in a temp folder joined with `Aegis Data` (created),
    `settings_file(.., true, true)` is `<that>/claude-statusline.json` and the file holds what `settings_arg` wrote;
    `settings_arg` gives the same path in double quotes; `mcp_file` likewise `<that>/mcp.json`.
  - `workspace/tests/usage.rs` `new_terminals_carry_what_aegis_types_after_claude_and_codex`, with the data folder
    `<tempdir>/Aegis Data` (`open_in(cx, None, Some(..))`); `word(k)` = that variable's value in `ws.agent_env()`;
    `p(name)` = the data folder joined with name, as a string with `\` as `/` on Windows:
    - `word("AEGIS_CLAUDE_ARGS") == format!("--settings\n{}\n--mcp-config={}", p("claude-statusline.json"), p("mcp.json"))`;
    - on Unix, `word("AEGIS_CODEX_ARGS") == codex_args(&program().unwrap().to_string_lossy(), false, true).unwrap().join("\n")`;
    - `config.connections.enabled = false`: Codex's `""`, Claude Code's `"--settings\n" + p("claude-statusline.json")`;
      then also `config.usage.claude_limits = false`: both `""`;
    - back on, shell `powershell`, then `CMD.EXE`: Codex's `""`, Claude Code's as in the first line; `WSL.exe`: both `""`;
    - `open_in(cx, None, None)` (no data folder): both variables present and `""`.
  - `workspace/tests/bus.rs`: `without_a_bus_terminals_name_none`'s script also prints
    `claude=%s;` of `${AEGIS_CLAUDE_ARGS-unset}` and the screen shows `claude=;`; 
    `terminals_get_the_bus_and_this_aegis_first_on_their_path` also prints the first line of
    `AEGIS_CLAUDE_ARGS` (`${AEGIS_CLAUDE_ARGS%%<newline>*}`) and the screen shows `--settings`.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p aegis-mcp codex_args && cargo test -p aegis-usage settings_file && cargo test -p aegis new_terminals_carry && cargo test -p aegis terminals_get_the_bus without_a_bus`
Expected: FAIL (the functions don't exist; the variables aren't set).

- [ ] **Step 3: Implement the interfaces above.** `claude_args` turns the paths into strings with `\` as `/` on
  Windows. `spawn_block` extends both arms of its `env` (the bus's, and the empty `AEGIS_BUS` one) with
  `self.agent_env()`. Doc comments say why both variables are always set.

- [ ] **Step 4: Run the tests of Step 2, then the full check command.** Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/aegis-mcp/src/lib.rs crates/aegis-usage/src/claude.rs crates/aegis/src/workspace/usage.rs \
  crates/aegis/src/workspace/blocks.rs crates/aegis/src/workspace/tests/usage.rs crates/aegis/src/workspace/tests/bus.rs
git commit -m "refactor(connections): build the words typed after claude and codex as arguments, and give them to new terminals"
```

---

### Task 2: `claude` and `codex` in bash and zsh

**Files:**
- Modify: `crates/aegis-terminal/src/integration/aegis.bash` (inside the `__aegis_installed` block)
- Modify: `crates/aegis-terminal/src/integration/zsh/.zshenv` (inside its interactive block)
- Modify: `crates/aegis/src/settings/connections.rs:46-49` (label and description), `crates/aegis/src/config.rs`
  (the `[connections] enabled` doc comment and the template's two comment lines), and any test naming the old label
- Modify: `docs/specs/settings.md:130` (the row), `docs/specs/connections.md` (the Config block's comment)
- Test: `crates/aegis-terminal/src/integration.rs` (`mod shells`)

**Interfaces:**
- Consumes: the variables of Task 1 (the tests set them by hand).
- Produces, for Task 3's tests: in `mod shells`,
  - `Home::spawn_with(&self, program: &str, a: &[&str], env: &[(&str, &str)]) -> Option<Terminal>` (`spawn` calls it
    with none; `env` goes on the `env` command line). It always sets `AEGIS_CLAUDE_ARGS` and `AEGIS_CODEX_ARGS`,
    to `""` unless `env` gives them: `cargo test` run in a Aegis terminal would hand the shell its own;
  - `Home::fake_agents(&self) -> String`: writes `<home>/bin/claude` and `<home>/bin/codex`, executable `sh` scripts
    that write each argument on its own line to `<home>/<name>.args` and exit 3, and returns `PATH=<home>/bin:<PATH>`'s
    value;
  - `fn wait_args(term: &Terminal, home: &Home, name: &str, want: &[&str])`: waits (the module's `wait`) until
    `<home>/<name>.args` holds exactly `want`, one per line, then deletes it;
  - `const CLAUDE_WORDS: &str = "--settings\n/tmp/Aegis Data/claude-statusline.json\n--mcp-config=/tmp/Aegis Data/mcp.json"`
    and `const CODEX_WORDS: &str = "-c\nmcp_servers.aegis.command=\"/opt/aegis\"\n-c\nhooks.Stop=[{hooks=[{type=\"command\",command=\"aegis hook codex\",timeout=5}]}]"`;
  - `fn typed_agents(term: &Terminal, home: &Home, unset: &str, status: bool)`: the plain session below, `unset`
    being the shell's line that empties the variable (`unset AEGIS_CLAUDE_ARGS`; Task 3 passes fish's and
    PowerShell's), `status` whether to check the exit code (bash: `bash_has_ps0()`, as `check` does; 3.2 has no PS0).
    It first waits for the startup folder to be reported, as `check` does, and types its first line with
    `wait_pressing(.., Some(b"\r"))`: PowerShell can drop the first Enter.

- [ ] **Step 1: Write the failing tests** (each shell skipped when not installed, as the module does). `W(s)` = the
  words of `s`, one per line. Typed lines end with `\r`.

  `typed_agents`, in a shell started with `fake_agents()`'s PATH, `AEGIS_CLAUDE_ARGS=CLAUDE_WORDS`,
  `AEGIS_CODEX_ARGS=CODEX_WORDS`, and no startup file of the person's but the `cd`:

  | Typed | `claude.args` / `codex.args` |
  |---|---|
  | `claude` | `W(CLAUDE_WORDS)`, and with `status` the command's end has code 3 (`term.status().last`) |
  | `claude hi ''` | `W(CLAUDE_WORDS)`, `hi`, `` (an empty line) |
  | `claude --settings x` | `--settings`, `x` |
  | `claude --mcp-config=y hi` | `--mcp-config=y`, `hi` |
  | `codex resume --last` | `W(CODEX_WORDS)`, `resume`, `--last` |
  | `codex -c 'mcp_servers.aegis.command="z"'` | `-c`, `mcp_servers.aegis.command="z"` |
  | the `unset` line, then `claude hi` | `hi` |

  Tests (in `mod shells`):
  - `bash_gives_typed_agents_aegiss_words` and `zsh_gives_typed_agents_aegiss_words`: `typed_agents` (zsh with
    `status` true).
  - `bash_keeps_the_persons_alias_and_function`: `.bashrc` has `alias claude='claude --model opus'` and
    `codex() { command codex mine "$@"; }` before the `cd`; `claude hi` gives `W(CLAUDE_WORDS)`, `--model`, `opus`,
    `hi`; `codex hi` gives `mine`, `hi`.
  - `zsh_keeps_the_persons_alias_and_function`: the same lines in `.zshrc`, same results.
  - `bash_without_the_variables_has_no_function`: started without the two variables; `claude hi` gives `hi`, and
    `codex hi` gives `hi`.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p aegis-terminal typed_agents keeps_the_persons without_the_variables`
Expected: FAIL (the args files lack Aegis's words). A shell that isn't installed is skipped and proves nothing:
install the missing ones without sudo first, into a temporary folder put first on `PATH` for the run (zsh: `brew
install zsh`, Homebrew is on this machine's PATH; fish: its static Linux tarball; pwsh: its GitHub release,
`powershell-<version>-linux-x64.tar.gz`), and say in the commit message which shells ran.

- [ ] **Step 3: Write the functions.** bash: after the startup files, for each of `claude` and `codex`, define it
  with the `function` keyword when its variable is non-empty and `declare -F` finds no function of that name. zsh:
  the same in the interactive block, with `$+functions[claude]`. At each call: read the variable; empty, or a word of
  the person's that the Global Constraints' rule names, run `command <name> "$@"`; else split the variable on `\n`
  (bash: `IFS=$'\n' builtin read -r -d '' -a words <<< "$w"`; zsh: `"${(@f)…}"`) and run
  `command <name> "${words[@]}" "$@"`. Quote the variable in every assignment: `builtin local w="${AEGIS_CLAUDE_ARGS-}"`
  (after `builtin`, bash splits an unquoted one into `local`'s arguments; seen while planning). A comment at the
  top of each block names `docs/specs/connections.md` ("Agents typed in a terminal").

- [ ] **Step 4: The switch's words.** Label, description and comments as the Global Constraints give them, in
  `settings/connections.rs`, `config.rs`, `docs/specs/settings.md` and connections.md's Config block.

- [ ] **Step 5: Run the tests of Step 2, then the full check command.** Expected: PASS (no shell skipped on this
  machine but those not installed).

- [ ] **Step 6: Commit**

```bash
git add crates/aegis-terminal/src/integration/aegis.bash crates/aegis-terminal/src/integration/zsh/.zshenv \
  crates/aegis-terminal/src/integration.rs crates/aegis/src/settings/connections.rs crates/aegis/src/config.rs \
  docs/specs/settings.md docs/specs/connections.md
git commit -m "feat(connections): a claude or codex typed in a bash or zsh terminal gets Aegis's server and hooks"
```

---

### Task 3: fish and PowerShell, and the docs

**Files:**
- Modify: `crates/aegis-terminal/src/integration/aegis.fish`, `crates/aegis-terminal/src/integration/aegis.ps1`
- Modify: `docs/specs/connections.md` (drop "planned" and "yet": Status, criterion 10, the line after Codex's typing
  rules, the section's first line becomes "Implemented: …", Testing's heading), `docs/specs/shell-integration.md` (drop
  "planned"), `docs/ARCHITECTURE.md` (Configuration: `[connections] enabled` covers the agents typed in its terminals;
  `aegis-terminal`'s integration bullet names the two functions)
- Test: `crates/aegis-terminal/src/integration.rs` (`mod shells`)

**Interfaces:**
- Consumes: Task 2's `spawn_with`, `fake_agents`, `wait_args`, `typed_agents`, `CLAUDE_WORDS`, `CODEX_WORDS`.

- [ ] **Step 1: Write the failing tests**
  - `fish_gives_typed_agents_aegiss_words`: `typed_agents` with `set -e AEGIS_CLAUDE_ARGS`, `status` true.
  - `powershell_gives_typed_agents_aegiss_words` (`pwsh -NoLogo`): `typed_agents` with
    `Remove-Item Env:AEGIS_CLAUDE_ARGS`, `status` true; the `claude hi ''` line keeps its empty argument as
    `claude hi ""`. The code 3 is the point of `$Global:__AegisAgentExit` below: without it the prompt says 0.
  - `fish_keeps_the_persons_function`: `config.fish` has `function codex; command codex mine $argv; end` and
    `alias claude 'claude --model opus'`; `codex hi` gives `mine`, `hi`; `claude hi` gives `--model`, `opus`, `hi`
    (fish's alias is a function: the person's wins).
  - `powershell_keeps_the_persons_function_and_alias`: the profile has `function codex { & (Get-Command codex
    -CommandType Application | Select-Object -First 1) mine @args }` and `Set-Alias claude <home>/bin/claude`;
    `codex hi` gives `mine`, `hi`; `claude hi` gives `hi`.

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p aegis-terminal fish_ powershell_`
Expected: FAIL, with fish and pwsh installed as Task 2's Step 2 says (a skipped shell proves nothing).

- [ ] **Step 3: Write the functions.** fish: in the interactive block, `function claude --wraps claude` when
  `$AEGIS_CLAUDE_ARGS` is non-empty and `functions -q claude` is false; words by `string split \n --`; the
  program by `command claude`. PowerShell: `function Global:claude` when `$env:AEGIS_CLAUDE_ARGS` is non-empty and
  neither `Function:\claude` nor `Alias:\claude` exists; the program is `Get-Command claude -CommandType
  Application, ExternalScript | Select-Object -First 1` (several on PATH give an array, which `&` can't run); words by
  `-split "`n"` without empty ones; `& $program @words @args`. Same rules for `codex`, and the Global Constraints' rule
  for adding nothing. A function's caller sees `$?` true whatever the program's exit code, so the function then sets
  `$Global:__AegisAgentExit = $LASTEXITCODE`, and the wrapped `prompt`, when it is set, reports it as the command's
  code (0 or not) and clears it.

- [ ] **Step 4: The docs** listed under Files.

- [ ] **Step 5: Run the tests of Step 2, then the full check command.** Expected: PASS.

- [ ] **Step 6: Check by hand** (Linux, `cargo run -p aegis -- --dev`): in a new bash terminal, `claude`: `/mcp`
  lists `aegis` connected, the dot turns accent on a prompt and back at its end, another agent's `delegate` to it
  answers; `codex`: `/mcp` lists it, Codex asks once to trust the hooks; `claude mcp list` and `codex resume --last`
  still work; `command claude`: no `aegis` in `/mcp`. Say in the commit message what was checked; Windows (Git Bash,
  pwsh 7, Windows PowerShell) and macOS stay unchecked, as the other connection checks.

- [ ] **Step 7: Commit**

```bash
git add crates/aegis-terminal/src/integration/aegis.fish crates/aegis-terminal/src/integration/aegis.ps1 \
  crates/aegis-terminal/src/integration.rs docs/specs/connections.md docs/specs/shell-integration.md docs/ARCHITECTURE.md
git commit -m "feat(connections): a claude or codex typed in a fish or PowerShell terminal gets Aegis's server and hooks"
```
