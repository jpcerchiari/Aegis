# Changelog

## 0.2.0

- **Pair a phone with two QR codes.** *Settings ▸ API ▸ Pair a phone…* asks what the phone's token can do (90 days
  by default) and at which address the phone reaches this computer, Tailscale's first so it works away from home.
  The first code installs Aegis's authority, from a page that exists only while the dialog is open and shows the
  fingerprint to compare; the second opens Aegis Mobile, which trades a one-time code for its token and keeps it.
  *Forget this phone* in the app removes it.
- **Aegis is its own certificate authority.** With *Aegis's certificate*, Aegis makes an authority once and issues
  with it the API's certificate for `localhost` and this computer's LAN and Tailscale addresses, again whenever they
  change. Install the authority on a phone once (*Save certificate…* now saves it), and its browser opens Aegis at
  those addresses without a warning. The authority can only vouch for `localhost` and private addresses, never for a
  site on the internet.
- **Shares made *Directly* need a new code, once.** They pinned the old certificate; Aegis to Aegis now pins the
  authority. Share again and paste the new code.

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
