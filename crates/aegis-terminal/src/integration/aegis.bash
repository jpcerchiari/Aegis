# Aegis's shell integration for bash, started with `--init-file` this file. It tells Aegis
# when a prompt starts (OSC 133 A), when a command starts and ends (C and D, with the exit code)
# and the current folder (OSC 7), and changes nothing else. Bash 4.4 or newer reports commands;
# any bash, prompts and the folder.
# The startup files part follows VS Code's shellIntegration-bash.sh (MIT, (c) Microsoft).

# --init-file took the place of the startup files: read the ones bash would have read.
if [[ -n "${AEGIS_SHELL_LOGIN-}" ]]; then
    builtin unset AEGIS_SHELL_LOGIN
    [[ -r /etc/profile ]] && builtin . /etc/profile
    for __aegis_file in ~/.bash_profile ~/.bash_login ~/.profile; do
        if [[ -r "$__aegis_file" ]]; then
            builtin . "$__aegis_file"
            break
        fi
    done
    builtin unset __aegis_file
else
    [[ -r /etc/bash.bashrc ]] && builtin . /etc/bash.bashrc
    [[ -r ~/.bashrc ]] && builtin . ~/.bashrc
fi

if [[ -z "${__aegis_installed-}" ]]; then
    __aegis_installed=1

    # First in PROMPT_COMMAND, for the command's status; it gives it back to what runs next.
    # The prompt is printed after PROMPT_COMMAND: A marks its start.
    __aegis_precmd() {
        builtin local status=$?
        builtin local folder="$PWD"
        if [[ -n "${WSL_DISTRO_NAME-}" && "${AEGIS_WSL_BASH-}" == "${BASH_SOURCE[0]}" ]]; then
            if [[ "${__aegis_wsl_pwd-}" != "$PWD" ]]; then
                __aegis_wsl_pwd="$PWD"
                __aegis_wsl_folder=$(builtin command wslpath -m "$PWD" 2>/dev/null) || __aegis_wsl_folder=
            fi
            folder="${__aegis_wsl_folder-}"
            [[ "$folder" != [A-Za-z]:/* ]] || folder="/$folder"
        fi
        builtin printf '\e]133;D;%s\a' "$status"
        [[ -z "$folder" ]] || builtin printf '\e]7;file://%s%s\a' "${HOSTNAME-}" "${folder//\%/%25}"
        builtin printf '\e]133;A\a'
        __aegis_ps0
        builtin return "$status"
    }
    # PS0 marks a command's start; a prompt theme may set it again later.
    __aegis_ps0() {
        [[ "${PS0-}" == *'\e]133;C\a'* ]] || PS0="${PS0-}"'\e]133;C\a'
    }
    __aegis_ps0
    if [[ -n "${BLE_VERSION-}" ]]; then
        # ble.sh, a line editor, puts PROMPT_COMMAND back as it was when it attaches, and draws
        # its first prompt before its hooks run: its own hook, and a first report now.
        blehook PRECMD+=__aegis_precmd
        __aegis_precmd
    else
        PROMPT_COMMAND="__aegis_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
    fi

    # A `claude`, `codex` or `copilot` typed here gets Aegis's MCP server and hooks: the words of
    # AEGIS_CLAUDE_ARGS, AEGIS_CODEX_ARGS or AEGIS_COPILOT_ARGS, one per line, before the
    # person's, unless they name Aegis's already (docs/specs/connections.md, "Agents typed in a
    # terminal"). Not over the person's own function; `function` keeps an alias of that name from
    # breaking this.
    if [[ -n "${WSL_DISTRO_NAME-}" && "${AEGIS_WSL_BASH-}" == "${BASH_SOURCE[0]}" && -r "${AEGIS_WSL_CLAUDE_SETTINGS-}" ]]; then
        AEGIS_CLAUDE_ARGS=$'--settings\n'"$AEGIS_WSL_CLAUDE_SETTINGS"
        if [[ -n "${AEGIS_WSL_CLAUDE_MCP-}" && -r "$AEGIS_WSL_CLAUDE_MCP" ]]; then
            AEGIS_CLAUDE_ARGS+=$'\n--mcp-config='"$AEGIS_WSL_CLAUDE_MCP"
        fi
    fi
    __aegis_agent() {
        builtin local name="$1" w="$2" words
        # A variable for this command only, unless the shell has one (the block Aegis opened,
        # the person's own).
        if [[ -n "$4" ]]; then
            [[ -n "${!3-}" ]] || builtin local -x "$3=$4"
        fi
        builtin shift 4
        if [[ -z "$w" ]]; then
            builtin command "$name" "$@"
        else
            # `read -d ''` fails at the end of its input: not a failure for `set -e`.
            IFS=$'\n' builtin read -r -d '' -a words <<< "$w" || builtin true
            builtin command "$name" "${words[@]}" "$@"
        fi
    }
    if [[ -n "${AEGIS_CLAUDE_ARGS-}" ]] && ! builtin declare -F claude > /dev/null; then
        function claude {
            builtin local w="${AEGIS_CLAUDE_ARGS-}" a
            for a in "$@"; do
                case "$a" in --settings | --settings=* | --mcp-config | --mcp-config=*) w= ;; esac
            done
            __aegis_agent claude "$w" "" "" "$@"
        }
    fi
    if [[ -n "${AEGIS_CODEX_ARGS-}" ]] && ! builtin declare -F codex > /dev/null; then
        function codex {
            builtin local w="${AEGIS_CODEX_ARGS-}" a
            for a in "$@"; do
                case "$a" in mcp_servers.aegis.*) w= ;; esac
            done
            __aegis_agent codex "$w" "" "" "$@"
        }
    fi
    # The others' words end with a path in Aegis's folder, which only Aegis types: none when
    # the person's words hold it (docs/specs/connections.md, "Typed by hand: the shells").
    if [[ -n "${AEGIS_COPILOT_ARGS-}" ]] && ! builtin declare -F copilot > /dev/null; then
        function copilot {
            builtin local w="${AEGIS_COPILOT_ARGS-}" nl=$'\n' last a
            # Not before a subcommand (`copilot plugin list`) nor for the help or the version: AWS's
            # `copilot` takes those too, and refuses Aegis's words.
            case "${1--}" in -*) ;; *) w= ;; esac
            last="${w##*$nl}"
            for a in "$@"; do
                case "$a" in "$last" | -h | --help | -v | --version) w= ;; esac
            done
            __aegis_agent copilot "$w" "" "" "$@"
        }
    fi
    # OpenCode and Kilo Code take Aegis's server and plugin from a variable.
    if [[ -n "${AEGIS_OPENCODE_CONFIG-}" ]] && ! builtin declare -F opencode > /dev/null; then
        function opencode {
            __aegis_agent opencode "" OPENCODE_CONFIG_CONTENT "${AEGIS_OPENCODE_CONFIG-}" "$@"
        }
    fi
    if [[ -n "${AEGIS_KILO_CONFIG-}" ]] && ! builtin declare -F kilo > /dev/null; then
        function kilo {
            __aegis_agent kilo "" KILO_CONFIG_CONTENT "${AEGIS_KILO_CONFIG-}" "$@"
        }
    fi
    # Qwen Code: its server as a word, its hooks in the settings file its variable moves. It takes
    # one --mcp-config: a person's own means none of Aegis's. None before one of its commands,
    # which refuse it (`qwen mcp list`); a plain word that isn't one is a prompt.
    if [[ -n "${AEGIS_QWEN_ARGS-}" ]] && ! builtin declare -F qwen > /dev/null; then
        function qwen {
            builtin local w="${AEGIS_QWEN_ARGS-}" a
            case "${1-}" in
                agents | auth | batch | board | channel | extensions | hooks | mcp | review | sandbox | \
                    serve | sessions | update) w= ;;
            esac
            for a in "$@"; do
                case "$a" in --mcp-config | --mcp-config=*) w= ;; esac
            done
            __aegis_agent qwen "$w" QWEN_CODE_SYSTEM_SETTINGS_PATH "${AEGIS_QWEN_SETTINGS-}" "$@"
        }
    fi
    # Pi: its extension. Not before one of its commands, which it reads only as the first word.
    if [[ -n "${AEGIS_PI_ARGS-}" ]] && ! builtin declare -F pi > /dev/null; then
        function pi {
            builtin local w="${AEGIS_PI_ARGS-}" nl=$'\n' last a
            case "${1-}" in install | remove | uninstall | update | list | config | auth) w= ;; esac
            last="${w##*$nl}"
            for a in "$@"; do
                case "$a" in "$last") w= ;; esac
            done
            __aegis_agent pi "$w" "" "" "$@"
        }
    fi

    # An account's command (docs/specs/accounts.md, "Typed in a terminal"): AEGIS_ACCOUNTS has a
    # line per account, its id, program, variable and folder separated by tabs. Its function runs
    # the program by its name, so the function above applies when there is one, with the variable
    # set to the folder for that command only; unless the person has a function of that name.
    # Only the id, which is a lowercase word, is in the text `eval` reads: the rest is read from
    # the line when the command runs.
    __aegis_account() {
        builtin local id="$1" aid prog var folder
        builtin shift
        while IFS=$'\t' builtin read -r aid prog var folder; do
            [[ "$aid" == "$id" ]] && break
            aid=
        done <<< "${AEGIS_ACCOUNTS-}"
        if [[ -z "$aid" ]]; then
            builtin printf 'bash: %s: command not found\n' "$id" >&2
            builtin return 127
        fi
        [[ "$var" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || builtin return 1
        # An account deleted since this terminal started has no folder: its command on the PATH,
        # which went with it. Aegis makes the folder of every account at each start.
        [[ -d "$folder" ]] || { builtin command "$id" "$@"; builtin return; }
        builtin local -x "$var=$folder"
        "$prog" "$@"
    }
    __aegis_accounts() {
        builtin local id rest re='^[a-z][a-z0-9_-]{0,31}$'
        while IFS=$'\t' builtin read -r id rest; do
            [[ "$id" =~ $re ]] || continue
            ! builtin declare -F -- "$id" > /dev/null || continue
            builtin eval "function $id { __aegis_account $id \"\$@\"; }"
        done <<< "${AEGIS_ACCOUNTS-}"
    }
    __aegis_accounts
    builtin unset -f __aegis_accounts
fi
