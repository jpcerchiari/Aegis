const byId = (id) => document.getElementById(id);
const connectForm = byId("connect");
const tokenField = byId("token");
const inputForm = byId("input");
const textField = byId("text");
const rows = byId("rows");
const cursor = byId("cursor");
const labels = {
    needs_you: "Needs you",
    starting: "Starting",
    waiting_subagents: "Waiting for subagents",
    working: "Working",
    done: "Done",
    idle: "Idle",
};

let session;
let terminal;

// The token stays on this phone until Forget this phone, or until Aegis refuses it.
const saved = "aegis.token";
const revoked = "This phone's token expired or was revoked. Pair it again: in Aegis, Settings ▸ API ▸ Pair a phone…";

function remembered() {
    try {
        return localStorage.getItem(saved) || "";
    } catch {
        return "";
    }
}

function remember(token) {
    try {
        localStorage.setItem(saved, token);
    } catch {
        // Storage refused (a private window): the token lasts as long as the page.
    }
}

function forgetToken() {
    try {
        localStorage.removeItem(saved);
    } catch {
        // Nothing was kept.
    }
}

function refused() {
    forgetToken();
    disconnect(revoked);
}

async function request(path, current, signal, input) {
    const headers = { Authorization: `Bearer ${current.token}` };
    if (input) headers["Content-Type"] = "application/json";
    const response = await fetch(path, {
        headers,
        signal,
        method: input ? "POST" : "GET",
        body: input ? JSON.stringify(input) : undefined,
        credentials: "omit",
        redirect: "error",
        mode: "same-origin",
    });
    if (response.ok) return response;
    const body = await response.json().catch(() => null);
    throw Object.assign(new Error(body?.error?.message || `Aegis returned HTTP ${response.status}.`), {
        status: response.status,
        retryAfter: Number(response.headers.get("Retry-After")),
    });
}

// Aegis writes one LF-terminated event name and one JSON data line per frame.
async function stream(path, current, signal, receive) {
    const response = await request(path, current, signal);
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let pending = "";
    try {
        while (!signal.aborted) {
            const { done, value } = await reader.read();
            if (done) break;
            pending += decoder.decode(value, { stream: true });
            let end;
            while (!signal.aborted && (end = pending.indexOf("\n\n")) !== -1) {
                const lines = pending.slice(0, end).split("\n");
                pending = pending.slice(end + 2);
                const event = lines.find((line) => line.startsWith("event: "));
                const data = lines.find((line) => line.startsWith("data: "));
                if (event && data) receive(event.slice(7), JSON.parse(data.slice(6)));
            }
        }
    } finally {
        await reader.cancel().catch(() => {});
        reader.releaseLock();
    }
}

async function watch(path, current, signal, receive, interrupted) {
    while (!signal.aborted) {
        let failure;
        try {
            await stream(path, current, signal, receive);
        } catch (error) {
            failure = error;
        }
        if (signal.aborted) return;
        if (failure?.status === 401) {
            refused();
            return;
        }
        const stopped = [403, 404, 422].includes(failure?.status);
        interrupted(stopped ? failure.message : "Connection lost. Reconnecting…");
        if (stopped) return;
        const seconds = Math.max(2, failure?.retryAfter || 0);
        await new Promise((resume) => setTimeout(resume, seconds * 1000));
    }
}

function overview() {
    const list = byId("terminals");
    const focused = document.activeElement?.dataset.block;
    const blocks = [...session.blocks.values()].filter((block) => block.kind === "terminal");
    blocks.sort((a, b) => Number(Boolean(b.agent)) - Number(Boolean(a.agent)));
    byId("empty").hidden = blocks.length > 0;
    byId("empty").textContent = "There are no terminals in the workspaces this token can see.";
    const items = blocks.map((block) => {
        const li = document.createElement("li");
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.block = block.id;
        const title = document.createElement("strong");
        title.textContent = block.title || "Terminal";
        const info = document.createElement("small");
        const dot = document.createElement("span");
        dot.className = "dot";
        dot.dataset.state = block.agent?.state || "";
        dot.setAttribute("aria-hidden", "true");
        const workspace = session.workspaces.get(block.workspace)?.name || "Workspace";
        const state = labels[block.agent?.state] || "Terminal";
        info.append(dot, `${workspace} · ${state}`);
        button.append(title, info);
        button.onclick = () => openTerminal(block.id);
        li.append(button);
        return li;
    });
    list.replaceChildren(...items);
    if (focused) items.find((item) => item.firstChild.dataset.block === focused)?.firstChild.focus();
}

function stateEvent(event, data) {
    if (event === "snapshot") {
        session.blocks = new Map(data.blocks.map((block) => [block.id, block]));
        session.workspaces = new Map(data.workspaces.map((workspace) => [workspace.id, workspace]));
        byId("connection").textContent = `Connected to ${session.me.computer} · ${session.me.scope}`;
    } else if (event === "block" || event === "workspace") {
        const collection = event === "block" ? session.blocks : session.workspaces;
        if (data.op === "put") collection.set(data.id, data.value);
        if (data.op === "delete") collection.delete(data.id);
    } else {
        return;
    }
    if (terminal) {
        const block = session.blocks.get(terminal.id);
        if (block?.kind === "terminal") updateControls(block);
        else showOverview("This terminal is no longer available.");
    }
    overview();
}

function closeTerminal() {
    terminal?.controller.abort();
    terminal = undefined;
    rows.replaceChildren();
    cursor.hidden = true;
    textField.value = "";
    byId("input-status").textContent = "";
    byId("detail").hidden = true;
}

function showOverview(message) {
    const previous = terminal?.id;
    closeTerminal();
    byId("overview").hidden = false;
    if (message) byId("connection").textContent = message;
    const buttons = byId("terminals").querySelectorAll("button");
    const opener = [...buttons].find((button) => button.dataset.block === previous);
    (opener || buttons[0] || byId("disconnect")).focus();
}

function updateControls(block) {
    byId("terminal-title").textContent = block.title || "Terminal";
    const canWrite = session.me.scope === "full" && !block.read_only;
    inputForm.hidden = !canWrite;
    byId("read-only").hidden = canWrite;
    for (const button of inputForm.querySelectorAll("button")) {
        button.disabled = !terminal.ready || terminal.sending;
    }
    textField.readOnly = terminal.sending;
}

function openTerminal(id) {
    closeTerminal();
    const block = session.blocks.get(id);
    if (!block) return;
    const view = { id, controller: new AbortController(), ready: false, sending: false };
    terminal = view;
    byId("overview").hidden = true;
    byId("detail").hidden = false;
    byId("terminal-status").textContent = "Opening live screen…";
    updateControls(block);
    byId("terminal-title").focus();
    watch(`/v1/terminals/${encodeURIComponent(id)}/stream`, session, view.controller.signal, (event, frame) => {
        if (event !== "frame") return;
        draw(frame);
        if (!view.ready) {
            view.ready = true;
            byId("terminal-status").textContent = "Live screen";
            updateControls(session.blocks.get(id));
        }
    }, (message) => {
        view.ready = false;
        byId("terminal-status").textContent = message;
        updateControls(session.blocks.get(id));
    });
}

const basic = [
    "#171421", "#ed8296", "#96d6ba", "#f6d385", "#a7b9ff", "#dda0eb", "#8ed8e5", "#e4d9ef",
    "#8d819e", "#ff9bae", "#b1ead0", "#ffe3a4", "#c2ccff", "#efbbff", "#a8e9f4", "#f3edf9",
];

function color(value) {
    if (typeof value === "string") return /^#[0-9a-f]{6}$/i.test(value) ? value : "";
    if (!Number.isInteger(value) || value < 0 || value > 255) return "";
    if (value < 16) return basic[value];
    if (value >= 232) return `rgb(${Array(3).fill(8 + 10 * (value - 232)).join(" ")})`;
    const n = value - 16;
    const levels = [0, 95, 135, 175, 215, 255];
    return `rgb(${levels[Math.floor(n / 36)]} ${levels[Math.floor(n / 6) % 6]} ${levels[n % 6]})`;
}

function draw(frame) {
    if (frame.whole) {
        const screen = document.createDocumentFragment();
        for (let y = 0; y < frame.size.rows; y++) {
            const row = document.createElement("div");
            row.className = "row";
            screen.append(row);
        }
        rows.replaceChildren(screen);
        rows.style.minWidth = `${frame.size.cols}ch`;
    }
    for (const { y, runs } of frame.rows) {
        rows.children[y]?.replaceChildren(...runs.map((run) => {
            const span = document.createElement("span");
            span.textContent = run.text;
            const fg = color(run.fg);
            const bg = color(run.bg);
            if (run.inverse) span.className = "inverse";
            span.style.color = run.inverse ? bg : fg;
            span.style.backgroundColor = run.inverse ? fg : bg;
            if (run.bold) span.style.fontWeight = "bold";
            if (run.italic) span.style.fontStyle = "italic";
            if (run.underline) span.style.textDecoration = "underline";
            if (run.dim) span.style.opacity = ".65";
            return span;
        }));
    }
    cursor.hidden = !frame.cursor;
    if (frame.cursor) {
        cursor.style.top = `${frame.cursor.row * 18}px`;
        cursor.style.left = `${frame.cursor.col}ch`;
        cursor.dataset.shape = frame.cursor.shape;
    }
}

async function send(input, clearText = false) {
    const view = terminal;
    if (!view?.ready || view.sending || inputForm.hidden) return;
    view.sending = true;
    updateControls(session.blocks.get(view.id));
    const status = byId("input-status");
    status.textContent = "Sending…";
    try {
        const response = await request(
            `/v1/terminals/${encodeURIComponent(view.id)}/input`, session, view.controller.signal, input,
        );
        const answer = await response.json();
        if (view !== terminal) return;
        status.textContent = answer.text || "Sent.";
        if (clearText) textField.value = "";
    } catch (error) {
        if (view !== terminal) return;
        if (error.status === 401) refused();
        else status.textContent = error.status ? error.message : "Delivery could not be confirmed. Check the terminal before sending again.";
    } finally {
        if (view === terminal) {
            view.sending = false;
            updateControls(session.blocks.get(view.id));
        }
    }
}

inputForm.addEventListener("submit", (event) => {
    event.preventDefault();
    const text = textField.value;
    if (!text) return;
    if (event.submitter?.value === "type") {
        const keys = Array.from(text, (key) => ({ "\n": "enter", "\t": "tab", " ": "space" })[key] || key);
        if (keys.length > 256) {
            byId("input-status").textContent = "Type up to 256 characters at a time, or use Paste.";
            return;
        }
        send({ keys }, true);
    } else {
        send({ text }, true);
    }
});

for (const button of inputForm.querySelectorAll("[data-key]")) {
    button.onclick = () => send({ keys: [button.dataset.key] });
}

function disconnect(message = "") {
    session?.controller.abort();
    session = undefined;
    closeTerminal();
    byId("terminals").replaceChildren();
    byId("overview").hidden = true;
    byId("disconnect").hidden = true;
    byId("forget").hidden = true;
    byId("resume").hidden = !remembered();
    connectForm.hidden = false;
    tokenField.value = "";
    byId("connection").textContent = message;
    (byId("resume").hidden ? tokenField : byId("resume")).focus();
}

async function connect(token) {
    if (!token || session) return;
    const current = { token, controller: new AbortController(), blocks: new Map(), workspaces: new Map() };
    session = current;
    tokenField.value = "";
    byId("paste").disabled = true;
    byId("resume").disabled = true;
    byId("connection").textContent = "Connecting…";
    try {
        const response = await request("/v1/me", current, current.controller.signal);
        current.me = await response.json();
        remember(token);
        connectForm.hidden = true;
        byId("disconnect").hidden = false;
        byId("forget").hidden = false;
        byId("empty").textContent = "Waiting for terminals…";
        byId("empty").hidden = false;
        byId("overview").hidden = false;
        byId("disconnect").focus();
        watch("/v1/events", current, current.controller.signal, stateEvent, (message) => {
            byId("connection").textContent = message;
        });
    } catch (error) {
        if (error.status === 401 && token === remembered()) {
            refused();
            return;
        }
        disconnect(error.status ? error.message : "Could not reach Aegis. Check the connection and try again.");
    } finally {
        byId("paste").disabled = false;
        byId("resume").disabled = false;
    }
}

// The second QR code of Aegis's Pair a phone: a code that works once, for this phone's token.
async function pair(code) {
    byId("connection").textContent = "Pairing…";
    let body;
    try {
        const response = await fetch("/v1/pair", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ code }),
            credentials: "omit",
            redirect: "error",
            mode: "same-origin",
        });
        body = await response.json().catch(() => null);
        if (!response.ok || !body?.token) {
            disconnect(body?.error?.message || `Aegis returned HTTP ${response.status}.`);
            return;
        }
    } catch {
        disconnect("Could not reach Aegis. Check the connection, then scan the second QR code again.");
        return;
    }
    await connect(body.token);
}

connectForm.addEventListener("submit", (event) => {
    event.preventDefault();
    connect(tokenField.value.trim());
});

byId("resume").onclick = () => connect(remembered());
byId("disconnect").onclick = () => disconnect();
byId("forget").onclick = () => {
    if (!confirm("Forget this phone's token? Aegis keeps it until you revoke it in Settings ▸ API.")) return;
    forgetToken();
    disconnect("This phone forgot its token. Revoke it in Aegis too: Settings ▸ API.");
};
byId("back").onclick = () => showOverview();

// A code from the QR code leaves the address at once: it works once, and the history keeps it.
const code = new URLSearchParams(location.hash.slice(1)).get("code");
if (code) {
    history.replaceState(null, "", location.pathname + location.search);
    pair(code);
} else if (remembered()) {
    connect(remembered());
} else {
    byId("resume").hidden = true;
}
