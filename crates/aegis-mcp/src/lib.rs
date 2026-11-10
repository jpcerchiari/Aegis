//! The bus Aegis's blocks talk over (docs/specs/connections.md): JSON-RPC lines on a local
//! socket, the MCP tools, the agents' hook events and the `aegis` command line. No UI.

pub mod ask;
pub mod bus;
pub mod cli;
pub mod control;
pub mod hooks;
pub mod rpc;
pub mod tools;

pub use bus::{Bus, Hello, Message, Replier};
pub use hooks::AgentEvent;
pub use rpc::Incoming;
pub use tools::{Call, Reply};

/// Codex's hooks Aegis gives it, with each one's timeout in seconds: Codex clamps an
/// `Interrupt` hook's to 3, and says so at every start for a longer one.
const CODEX_HOOKS: [(&str, u8); 6] = [
    ("SessionStart", 5),
    ("UserPromptSubmit", 5),
    ("PermissionRequest", 5),
    ("PostToolUse", 5),
    ("Stop", 5),
    ("Interrupt", 3),
];

/// What the Codex Aegis opens is given (docs/specs/connections.md): Aegis's MCP server, the
/// variables it needs (Codex hands a server only those it is told to), and, when `hooks`
/// (`aegis` is on the terminal's PATH), its hooks, as `-c` flags to append to its command.
/// Their command is the bare `aegis hook codex`, `<data>/bin` first on PATH: Codex asks its
/// person to trust them once, whatever Aegis's path. None when `exe` can't go inside single
/// quotes and a TOML string.
pub fn codex_flags(exe: &str, windows: bool, hooks: bool) -> Option<String> {
    let words = codex_args(exe, windows, hooks)?;
    Some(
        words
            .chunks(2)
            .map(|flag| format!(" -c '{}'", flag[1]))
            .collect(),
    )
}

/// `codex_flags` as the words a program gets, `-c` and each value, unquoted: what a new terminal
/// hands the `codex` a person types (docs/specs/connections.md, Agents typed in a terminal).
pub fn codex_args(exe: &str, windows: bool, hooks: bool) -> Option<Vec<String>> {
    let exe = if windows {
        exe.replace('\\', "/")
    } else {
        exe.to_string()
    };
    if exe.is_empty() || exe.contains(['\'', '"', '\\', '\n']) {
        return None;
    }
    Some(codex_words(
        &exe,
        hooks.then_some("aegis hook codex"),
        false,
    ))
}

/// Codex's Linux process uses a shell to expand the translated path to the Windows bridge.
pub fn codex_wsl_args() -> Vec<String> {
    codex_words("/bin/sh", Some("exec \"$AEGIS_WSL_EXE\" hook codex"), true)
}

fn codex_words(exe: &str, hook: Option<&str>, wsl: bool) -> Vec<String> {
    let args = if wsl {
        r#"["-c","exec \"$AEGIS_WSL_EXE\" mcp"]"#
    } else {
        r#"["mcp"]"#
    };
    let env = if wsl {
        r#"["AEGIS_BUS","AEGIS_TERMINAL","AEGIS_WSL_EXE","WSLENV"]"#
    } else {
        r#"["AEGIS_BUS","AEGIS_TERMINAL"]"#
    };
    let mut values = vec![
        format!("mcp_servers.aegis.command=\"{exe}\""),
        format!("mcp_servers.aegis.args={args}"),
        format!("mcp_servers.aegis.env_vars={env}"),
        "mcp_servers.aegis.tool_timeout_sec=604800".to_string(),
        // `reply` asks no permission, and only `reply` (docs/specs/agent-protocol.md, 9).
        "mcp_servers.aegis.tools.reply.approval_mode=\"approve\"".to_string(),
    ];
    if let Some(hook) = hook {
        let hook = serde_json::Value::String(hook.to_owned());
        for (event, timeout) in CODEX_HOOKS
            .into_iter()
            .chain(wsl.then_some(("SessionEnd", 5)))
        {
            values.push(format!(
                "hooks.{event}=[{{hooks=[{{type=\"command\",command={hook},timeout={timeout}}}]}}]"
            ));
        }
    }
    values
        .into_iter()
        .flat_map(|value| ["-c".to_string(), value])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wsl_codex_passes_wslenv_to_mcp_and_hooks_the_sessions_end() {
        let args = codex_wsl_args();
        assert!(args.contains(&r#"mcp_servers.aegis.command="/bin/sh""#.to_string()));
        assert!(args.iter().any(|arg| arg.starts_with("mcp_servers.aegis.env_vars=") && arg.contains("WSLENV")));
        assert!(
            args.iter()
                .any(|arg| arg.starts_with("hooks.SessionEnd=") && arg.contains("$AEGIS_WSL_EXE"))
        );
    }

    #[test]
    fn codex_gets_the_server_and_its_hooks_in_single_quotes() {
        let flags = codex_flags("/opt/Aegis App/aegis", false, true).unwrap();
        assert!(
            flags.starts_with(" -c 'mcp_servers.aegis.command=\"/opt/Aegis App/aegis\"'"),
            "{flags}"
        );
        assert!(flags.contains("-c 'mcp_servers.aegis.args=[\"mcp\"]'"));
        assert!(flags.contains("env_vars=[\"AEGIS_BUS\",\"AEGIS_TERMINAL\"]"));
        for event in [
            "SessionStart",
            "UserPromptSubmit",
            "PermissionRequest",
            "PostToolUse",
            "Stop",
        ] {
            let hook = format!(
                " -c 'hooks.{event}=[{{hooks=[{{type=\"command\",command=\"aegis hook codex\",timeout=5}}]}}]'"
            );
            assert!(flags.contains(&hook), "{event}: {flags}");
        }
        // Codex gives an Interrupt hook 3 s at most, and says so at every start otherwise.
        assert!(
            flags.ends_with(
                " -c 'hooks.Interrupt=[{hooks=[{type=\"command\",command=\"aegis hook codex\",timeout=3}]}]'"
            ),
            "{flags}"
        );
        assert!(!flags.contains("notify"), "the person's own notify runs");
        // `reply` asks no permission, and only `reply` (docs/specs/agent-protocol.md, 9).
        assert!(
            flags.contains(" -c 'mcp_servers.aegis.tools.reply.approval_mode=\"approve\"'"),
            "{flags}"
        );
        assert_eq!(flags.matches("approval_mode").count(), 1);
        // Without `aegis` on the terminal's PATH, the server only.
        let bare = codex_flags("/opt/aegis", false, false).unwrap();
        assert!(
            bare.ends_with("tools.reply.approval_mode=\"approve\"'") && !bare.contains("hooks."),
            "{bare}"
        );
        let windows = codex_flags(r"C:\Program Files\Aegis\aegis.exe", true, true).unwrap();
        assert!(
            windows.contains("command=\"C:/Program Files/Aegis/aegis.exe\""),
            "{windows}"
        );
        assert_eq!(codex_flags("/home/o'neil/aegis", false, true), None);
        assert_eq!(codex_flags("", false, true), None);
    }

    /// What a new terminal hands the `codex` a person types: the same values, one word each.
    #[test]
    fn codex_args_are_the_flags_unquoted() {
        let words = codex_args("/opt/Aegis App/aegis", false, true).unwrap();
        assert_eq!(words.len(), 22, "{words:?}");
        assert!(words.iter().step_by(2).all(|w| w == "-c"), "{words:?}");
        assert_eq!(
            words[1],
            "mcp_servers.aegis.command=\"/opt/Aegis App/aegis\""
        );
        assert_eq!(
            words[9],
            "mcp_servers.aegis.tools.reply.approval_mode=\"approve\""
        );
        assert_eq!(
            words[21],
            "hooks.Interrupt=[{hooks=[{type=\"command\",command=\"aegis hook codex\",timeout=3}]}]"
        );
        assert_eq!(codex_args("/opt/aegis", false, false).unwrap().len(), 10);
        assert_eq!(codex_args("/home/o'neil/aegis", false, true), None);
    }
}
