# Changelog

## 0.1.0

The first release of Aegis: terminals and AI coding agents on one infinite canvas, for macOS, Linux and Windows.

- **Agents in WSL.** Choose **WSL** as an agent's shell and open it from the canvas: Claude Code and Codex get
  Aegis's MCP server and hooks, and Claude Code its status line and usage meter. The canvas shows the agent's status,
  keeps its session and resumes it when Aegis opens again. `spawn_agent` can start Claude Code in WSL; Codex there
  takes its first prompt from you. Stop and accounts don't reach WSL yet.
- **Aegis on your phone, first version.** With the API on HTTPS, Aegis serves a web app at `/mobile/`: your agents
  with their status, and each terminal live; a `full` token lets you type into it. For now you set up the
  certificate and the token by hand; pairing with a QR code comes next.
- **Updates from Aegis's own releases.** Aegis checks this repository's releases, downloads the new version for your
  system and checks it against the release's SHA-256 before installing it.
- **A name and an icon of its own,** after the Aegis item in *Risk of Rain 2*.
