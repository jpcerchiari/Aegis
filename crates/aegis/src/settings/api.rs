//! Settings ▸ API (docs/specs/api.md, Settings ▸ API): the switch, who can reach it, HTTPS and
//! its certificate, the web pages allowed to call, the tokens and who is connected.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use aegis_api::store::{Listen, Scope, Tls, Token, Use, Way};
use chrono::{Local, NaiveDate};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Bounds, ClickEvent, Context, ElementId, Entity, SharedString, TestSupportExt, Window,
    canvas, div, fill, point, px, size,
};

use super::row::{Row, commit_input, menu_button, text_area};
use crate::grid::hsla;
use crate::prompt;
use crate::theme::Ui;
use crate::workspace::api::PAIRING_NEEDS;
use crate::workspace::{Workspace, show_notification};

/// The words of the line under HTTPS, when it is plain on this computer.
pub(crate) const PLAIN: &str =
    "Plain HTTP on this computer: give HTTPS with your tunnel or proxy (cloudflared does).";
pub(crate) const NEEDS_HTTPS: &str = "Off this computer, the API needs HTTPS.";
const FULL_WARNING: &str = "Anyone with a Full token can run commands on this computer.";
const SHOWN_ONCE: &str =
    "Copy it now: Aegis keeps only a fingerprint of it and can't show it again.";
const NO_CLIENT: &str = "No client is connected.";
const NO_ADDRESS: &str =
    "This computer has no address on a home network or Tailscale for a phone to reach.";

/// How long ago, in the largest unit that is at least 1: "3 min", "2 days".
pub(crate) fn span(secs: u64) -> String {
    let (n, unit) = match secs {
        0..=59 => (secs, "s"),
        60..=3599 => (secs / 60, " min"),
        3600..=86_399 => (secs / 3600, " h"),
        _ => (
            secs / 86_400,
            if secs / 86_400 == 1 { " day" } else { " days" },
        ),
    };
    format!("{n}{unit}")
}

/// "expires in 29 days", "never expires", "expired 2 days ago".
pub(crate) fn expiry_words(expires: Option<u64>, now: u64) -> String {
    match expires {
        None => "never expires".into(),
        Some(end) if end > now => format!("expires in {}", span(end - now)),
        Some(end) => format!("expired {} ago", span(now - end)),
    }
}

/// "last used 3 min ago from 203.0.113.7", "never used".
pub(crate) fn used_words(last: Option<&Use>, now: u64) -> String {
    match last {
        None => "never used".into(),
        Some(used) => format!(
            "last used {} ago from {}",
            span(now.saturating_sub(used.at)),
            used.addr
        ),
    }
}

/// What a share's token says it is in place of its workspaces: "share of API", or "share of a
/// deleted workspace" when the workspace it was made for is not among `names` (workspace id →
/// name). None for a token Settings made.
pub(crate) fn share_line(token: &Token, names: &HashMap<String, String>) -> Option<String> {
    token.share.as_ref()?;
    // The token's list is the truth: the workspace it was made for may be gone and its id used again.
    let live = token
        .workspaces
        .iter()
        .flatten()
        .find_map(|id| names.get(id));
    Some(match live {
        Some(name) => format!("share of {name}"),
        None => "share of a deleted workspace".into(),
    })
}

/// What New certificate's question says it breaks: every client that pinned the old authority or
/// installed it (a phone), and the `direct` mirrors that pinned it.
pub(crate) fn renew_words(direct: usize) -> String {
    let base = "Clients that pinned or installed the old one stop connecting";
    match direct {
        0 => format!("{base}."),
        1 => format!("{base}, 1 mirror connected directly among them: share again for it."),
        n => format!("{base}, {n} mirrors connected directly among them: share again for them."),
    }
}

/// The end of the day `YYYY-MM-DD`, local time, in Unix seconds.
pub(crate) fn end_of(date: &str) -> Option<u64> {
    let day = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").ok()?;
    let end = day
        .and_hms_opt(23, 59, 59)?
        .and_local_timezone(Local)
        .latest()?;
    u64::try_from(end.timestamp()).ok()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn pill(
    ui: Ui,
    id: &'static str,
    label: impl Into<SharedString>,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .px(px(10.0))
        .py(px(3.0))
        .rounded_full()
        .border_1()
        .border_color(ui.hairline)
        .text_size(px(12.0))
        .cursor_pointer()
        .hover(move |s| s.bg(ui.hover))
        .active(move |s| s.bg(ui.pressed))
        .child(label.into())
}

fn set_port(ws: &mut Workspace, text: &str, cx: &mut Context<Workspace>) -> Result<(), String> {
    match text.trim().parse::<u16>() {
        Ok(port) if port >= 1024 => {
            ws.api_edit(|s| s.port = port, cx);
            Ok(())
        }
        _ => Err("A port is a number from 1024 to 65535.".into()),
    }
}

fn set_address(ws: &mut Workspace, text: &str, cx: &mut Context<Workspace>) -> Result<(), String> {
    let text = text.trim().to_string();
    ws.api_edit(|s| s.listen = Listen::Address(text), cx);
    Ok(())
}

fn set_cert(ws: &mut Workspace, text: &str, cx: &mut Context<Workspace>) -> Result<(), String> {
    let text = text.trim().to_string();
    ws.api_edit(
        |s| {
            if let Tls::Files { cert, .. } = &mut s.tls {
                *cert = text.into();
            }
        },
        cx,
    );
    Ok(())
}

fn set_key(ws: &mut Workspace, text: &str, cx: &mut Context<Workspace>) -> Result<(), String> {
    let text = text.trim().to_string();
    ws.api_edit(
        |s| {
            if let Tls::Files { key, .. } = &mut s.tls {
                *key = text.into();
            }
        },
        cx,
    );
    Ok(())
}

fn set_origins(ws: &mut Workspace, text: &str, cx: &mut Context<Workspace>) {
    let origins: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();
    ws.api_edit(|s| s.origins = origins, cx);
}

pub(super) fn rows(
    ws: &Workspace,
    entity: &Entity<Workspace>,
    window: &mut Window,
    cx: &mut App,
) -> Vec<Row> {
    let ui = ws.ui;
    let red = hsla(ws.theme.palette.ansi[1], 1.0);
    let Some(store) = ws.api.store.as_ref() else {
        return vec![Row::new("api.none", "No API").description(
            "This Aegis runs without its data folder: the API keeps its tokens there",
        )];
    };
    let settings = &store.settings;
    let e = entity.clone();
    let mut rows = Vec::new();

    // The switch, and what it did.
    let mut on = Row::new("api.enabled", "Turn on the API")
        .description("Off until you turn it on; then it answers only to the tokens you make")
        .control(
            Switch::new("api-switch")
                .checked(settings.enabled)
                .on_click(move |on: &bool, _, cx| {
                    let on = *on;
                    e.update(cx, |ws, cx| ws.api_edit(|s| s.enabled = on, cx))
                }),
        );
    if settings.enabled {
        on = match (&ws.api.server, &ws.api.error) {
            (Some(server), _) => {
                let url = format!(
                    "{}://{}",
                    if ws.api_tls() { "https" } else { "http" },
                    server.addr()
                );
                on.below(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .text_size(px(12.0))
                        .child(
                            div()
                                .id("api-listening")
                                .child(format!("Listening on {url}"))
                                .test_support(),
                        )
                        .child(Clipboard::new("api-copy-url").value(url)),
                )
            }
            (None, Some(why)) => on.below(
                div()
                    .id("api-error")
                    .text_size(px(12.0))
                    .text_color(red)
                    .child(why.clone())
                    .test_support(),
            ),
            (None, None) => on,
        };
    }
    rows.push(on);

    // Who can reach it.
    let kind = match &settings.listen {
        Listen::Loopback => "loopback",
        Listen::All => "all",
        Listen::Address(_) => "address",
    };
    let e = entity.clone();
    let options = vec![
        ("loopback".into(), "This computer only (127.0.0.1)".into()),
        (
            "all".into(),
            "Every network of this computer (0.0.0.0)".into(),
        ),
        ("address".into(), "One address…".into()),
    ];
    let mut reach = Row::new("api.listen", "Who can reach it").control(menu_button(
        "menu-api-listen",
        options,
        kind.into(),
        false,
        move |value: SharedString, cx: &mut App| {
            e.update(cx, |ws, cx| {
                ws.api_edit(
                    |s| {
                        s.listen = match value.as_ref() {
                            "all" => Listen::All,
                            "address" => Listen::Address(String::new()),
                            _ => Listen::Loopback,
                        }
                    },
                    cx,
                )
            })
        },
    ));
    if let Listen::Address(address) = &settings.listen {
        reach = reach.below(commit_input(
            entity,
            "api-address",
            address.clone(),
            set_address,
            red,
            window,
            cx,
        ));
    }
    rows.push(reach);

    rows.push(
        Row::new("api.port", "Port")
            .description("1024–65535. A tunnel's config stays right with a fixed one")
            .control(commit_input(
                entity,
                "api-port",
                settings.port.to_string(),
                set_port,
                red,
                window,
                cx,
            )),
    );

    // HTTPS.
    let loopback = matches!(settings.listen, Listen::Loopback)
        || matches!(&settings.listen, Listen::Address(a) if a.trim().parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()));
    let tls_kind = match &settings.tls {
        Tls::Off => "off",
        Tls::Own => "own",
        Tls::Files { .. } => "files",
    };
    let mut tls_options: Vec<(SharedString, SharedString)> = Vec::new();
    if loopback {
        tls_options.push(("off".into(), "Off: a tunnel or proxy gives HTTPS".into()));
    }
    tls_options.push(("own".into(), "Aegis's certificate".into()));
    tls_options.push(("files".into(), "My certificate…".into()));
    let e = entity.clone();
    let words = if loopback && tls_kind == "off" {
        PLAIN
    } else if !loopback {
        NEEDS_HTTPS
    } else {
        ""
    };
    let mut https = Row::new("api.tls", "HTTPS")
        .description(words)
        .control(menu_button(
            "menu-api-tls",
            tls_options,
            tls_kind.into(),
            false,
            move |value: SharedString, cx: &mut App| {
                e.update(cx, |ws, cx| {
                    ws.api_edit(
                        |s| {
                            s.tls = match value.as_ref() {
                                "own" => Tls::Own,
                                "files" => Tls::Files {
                                    cert: Default::default(),
                                    key: Default::default(),
                                },
                                _ => Tls::Off,
                            }
                        },
                        cx,
                    )
                })
            },
        ));
    if let Tls::Files { cert, key } = &settings.tls {
        https = https.below(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(ui.title_dim)
                        .child("Certificate (PEM) and private key (PEM)"),
                )
                .child(commit_input(
                    entity,
                    "api-cert",
                    cert.display().to_string(),
                    set_cert,
                    red,
                    window,
                    cx,
                ))
                .child(commit_input(
                    entity,
                    "api-key",
                    key.display().to_string(),
                    set_key,
                    red,
                    window,
                    cx,
                )),
        );
    }
    rows.push(https);

    if matches!(settings.tls, Tls::Own) {
        rows.push(certificate_row(ws, entity, ui));
    }

    rows.push(
        Row::new("api.origins", "Allowed web pages")
            .description("One origin per line (https://dash.example.com). Empty: no web page may call the API")
            .below(text_area(entity, "api-origins", settings.origins.join("\n"), 3, set_origins, window, cx)),
    );

    // Tokens.
    let at = now();
    let names: HashMap<String, String> = ws
        .order()
        .iter()
        .map(|s| (s.id.clone(), s.name.clone()))
        .collect();
    for token in &store.tokens {
        let id = token.id;
        let e = entity.clone();
        let workspaces = share_line(token, &names).unwrap_or_else(|| match &token.workspaces {
            None => "all workspaces".to_string(),
            Some(list) => list
                .iter()
                .map(|id| names.get(id).cloned().unwrap_or_else(|| id.clone()))
                .collect::<Vec<_>>()
                .join(", "),
        });
        let expired = token.expires.is_some_and(|end| end <= at);
        let detail = format!(
            "{} · {} · {} · {}",
            token.scope.word(),
            workspaces,
            expiry_words(token.expires, at),
            used_words(token.last.as_ref(), at)
        );
        rows.push(
            Row::new(format!("api.token.{id}"), token.name.clone())
                .description(detail)
                .control(
                    pill(ui, "api-revoke", "Revoke")
                        .id(ElementId::Name(format!("api-revoke-{id}").into()))
                        .when(expired, |el| {
                            el.text_color(hsla(ws.theme.palette.ansi[3], 1.0))
                        })
                        .on_click(move |_: &ClickEvent, _, cx| {
                            e.update(cx, |ws, cx| ws.revoke_token(id, cx))
                        })
                        .test_support(),
                ),
        );
    }
    let e = entity.clone();
    rows.push(
        Row::new("api.new-token", "Tokens")
            .description("The keys a client sends as Authorization: Bearer. Shown once, kept as a fingerprint")
            .control(
                pill(ui, "api-new-token", "New token…")
                    .on_click(move |_: &ClickEvent, window, cx| e.update(cx, |ws, cx| token_dialog(ws, window, cx)))
                    .test_support(),
            ),
    );
    let e = entity.clone();
    rows.push(
        Row::new("api.pair", "Phone")
            .description("Aegis Mobile on a phone, by two QR codes: no token to type")
            .control(
                pill(ui, "api-pair", "Pair a phone…")
                    .on_click(move |_: &ClickEvent, window, cx| {
                        e.update(cx, |ws, cx| pair_dialog(ws, window, cx))
                    })
                    .test_support(),
            ),
    );
    if !store.tokens.is_empty() {
        let (e, n) = (entity.clone(), store.tokens.len());
        rows.push(
            Row::new("api.revoke-all", "Revoke all").control(
                pill(ui, "api-revoke-all", "Revoke all")
                    .on_click(move |_: &ClickEvent, window, cx| {
                        let e = e.clone();
                        prompt::confirm(
                            if n == 1 {
                                "Revoke all 1 token?".to_string()
                            } else {
                                format!("Revoke all {n} tokens?")
                            },
                            "Every client is cut at once.",
                            "Revoke all",
                            move |_, cx| e.update(cx, |ws, cx| ws.revoke_all_tokens(cx)),
                            window,
                            cx,
                        )
                    })
                    .test_support(),
            ),
        );
    }

    // Who is connected.
    let clients = ws
        .api
        .server
        .as_ref()
        .map(|s| s.clients())
        .unwrap_or_default();
    if clients.is_empty() {
        rows.push(Row::new("api.clients", "Connected now").description(NO_CLIENT));
    }
    for client in clients {
        let (e, id) = (entity.clone(), client.id);
        let listening = if client.streaming {
            " · listening to events"
        } else {
            ""
        };
        rows.push(
            Row::new(
                format!("api.client.{id}"),
                format!("{} · {}", client.name, client.addr),
            )
            .description(format!(
                "since {} ago{listening}",
                span(at.saturating_sub(client.since))
            ))
            .control(
                pill(ui, "api-disconnect", "Disconnect")
                    .id(ElementId::Name(format!("api-disconnect-{id}").into()))
                    .on_click(move |_: &ClickEvent, _, cx| {
                        e.update(cx, |ws, _| ws.disconnect_client(id))
                    })
                    .test_support(),
            ),
        );
    }
    rows
}

/// Aegis's certificate: its fingerprint to pin, a copy to install, and a new one.
fn certificate_row(ws: &Workspace, entity: &Entity<Workspace>, ui: Ui) -> Row {
    let fingerprint = ws.api.fingerprint.clone().unwrap_or_default();
    let (save, renew) = (entity.clone(), entity.clone());
    // The mirrors that pinned it: a live share made for Directly.
    let at = now();
    let direct = ws
        .api
        .store
        .iter()
        .flat_map(|s| s.tokens.iter())
        .filter(|t| {
            t.live(at)
                && t.share
                    .as_ref()
                    .is_some_and(|share| share.way == Way::Pinned)
        });
    let question = renew_words(direct.count());
    Row::new("api.certificate", "Certificate")
        .description(
            "Aegis's own authority, which signs this computer's certificate. Its SHA-256 \
             fingerprint: a client pins it. Install the .pem on a phone to trust it",
        )
        .below(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .id("api-fingerprint")
                                .flex_1()
                                .min_w(px(0.0))
                                .text_size(px(11.0))
                                .font_family(ws.cell.font.family.clone())
                                .overflow_hidden()
                                .child(fingerprint.clone())
                                .test_support(),
                        )
                        .child(Clipboard::new("api-copy-fingerprint").value(fingerprint)),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .child(
                            pill(ui, "api-save-certificate", "Save certificate…")
                                .on_click(move |_: &ClickEvent, window, cx| {
                                    save.update(cx, |ws, cx| ws.save_certificate(window, cx))
                                })
                                .test_support(),
                        )
                        .child(
                            pill(ui, "api-new-certificate", "New certificate")
                                .on_click(move |_: &ClickEvent, window, cx| {
                                    let e = renew.clone();
                                    prompt::confirm(
                                        "Make a new certificate?",
                                        question.clone(),
                                        "New certificate",
                                        move |_, cx| {
                                            e.update(cx, |ws, cx| ws.renew_certificate(cx))
                                        },
                                        window,
                                        cx,
                                    )
                                })
                                .test_support(),
                        ),
                ),
        )
}

/// How long a new token lives.
#[derive(Clone, Copy, PartialEq)]
enum Life {
    Days(u64),
    Date,
    Never,
}

/// What New token and Pair a phone hold.
struct Draft {
    scope: Scope,
    life: Life,
    /// None: all workspaces.
    workspaces: Option<Vec<String>>,
    /// Pair a phone's: the address the phone reaches this computer at.
    address: Option<Ipv4Addr>,
}

/// A field's name, over it.
fn label(ui: Ui, text: &'static str) -> gpui_kit::Div {
    div()
        .mt(px(4.0))
        .text_size(px(12.0))
        .text_color(ui.title_dim)
        .child(text)
}

/// When a token made now ends; Err when On a date… isn't YYYY-MM-DD.
fn expiry(life: Life, date: &str) -> Result<Option<u64>, String> {
    match life {
        Life::Never => Ok(None),
        Life::Days(n) => Ok(Some(now() + n * 86_400)),
        Life::Date => end_of(date)
            .map(Some)
            .ok_or_else(|| "Write the date as YYYY-MM-DD.".to_string()),
    }
}

/// The workspaces a token can be limited to: (id, name).
fn spaces(ws: &Workspace) -> Vec<(String, String)> {
    ws.order()
        .iter()
        .map(|s| (s.id.clone(), s.name.clone()))
        .collect()
}

/// What New token and Pair a phone share: what the token can do, when it expires and which
/// workspaces it sees.
fn token_fields(
    draft: &Entity<Draft>,
    entity: &Entity<Workspace>,
    spaces: &[(String, String)],
    date: &Entity<InputState>,
    ui: Ui,
    cx: &App,
) -> gpui_kit::Div {
    let current = draft.read(cx);
    let (scope, life, chosen) = (current.scope, current.life, current.workspaces.clone());
    let pick_scope = {
        let (draft, entity) = (draft.clone(), entity.clone());
        move |value: SharedString, cx: &mut App| {
            draft.update(cx, |d, _| {
                d.scope = match value.as_ref() {
                    "read" => Scope::Read,
                    "full" => Scope::Full,
                    _ => Scope::Control,
                }
            });
            entity.update(cx, |_, cx| cx.notify());
        }
    };
    let lives: Vec<(SharedString, SharedString)> = vec![
        ("1".into(), "In 1 day".into()),
        ("7".into(), "In 7 days".into()),
        ("30".into(), "In 30 days".into()),
        ("90".into(), "In 90 days".into()),
        ("365".into(), "In 1 year".into()),
        ("date".into(), "On a date…".into()),
        ("never".into(), "Never".into()),
    ];
    let life_key: SharedString = match life {
        Life::Days(n) => n.to_string().into(),
        Life::Date => "date".into(),
        Life::Never => "never".into(),
    };
    let pick_life = {
        let (draft, entity) = (draft.clone(), entity.clone());
        move |value: SharedString, cx: &mut App| {
            draft.update(cx, |d, _| {
                d.life = match value.as_ref() {
                    "date" => Life::Date,
                    "never" => Life::Never,
                    days => Life::Days(days.parse().unwrap_or(30)),
                }
            });
            entity.update(cx, |_, cx| cx.notify());
        }
    };
    let every = {
        let (draft, entity) = (draft.clone(), entity.clone());
        Checkbox::new("api-token-all-workspaces")
            .label("All workspaces")
            .checked(chosen.is_none())
            .on_click(move |on: &bool, _, cx| {
                let on = *on;
                draft.update(cx, |d, _| {
                    d.workspaces = if on { None } else { Some(Vec::new()) }
                });
                entity.update(cx, |_, cx| cx.notify());
            })
    };
    let boxes = spaces.iter().map(|(id, title)| {
        let (draft, entity, id) = (draft.clone(), entity.clone(), id.clone());
        let checked = chosen.as_ref().is_some_and(|c| c.contains(&id));
        Checkbox::new(ElementId::Name(format!("api-token-workspace-{id}").into()))
            .label(title.clone())
            .checked(checked)
            .on_click(move |on: &bool, _, cx| {
                let on = *on;
                draft.update(cx, |d, _| {
                    let list = d.workspaces.get_or_insert_with(Vec::new);
                    list.retain(|w| *w != id);
                    if on {
                        list.push(id.clone());
                    }
                });
                entity.update(cx, |_, cx| cx.notify());
            })
    });
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(label(ui, "Can"))
        .child(menu_button(
            "menu-api-token-scope",
            vec![
                (
                    "read".into(),
                    "Read: see everything, terminals' text included".into(),
                ),
                (
                    "control".into(),
                    "Control: also act, answer requests, open agents, run automations".into(),
                ),
                (
                    "full".into(),
                    "Full: also type into terminals and press keys".into(),
                ),
            ],
            scope.word().to_lowercase().into(),
            false,
            pick_scope,
        ))
        .when(scope == Scope::Full, |el| {
            el.child(
                div()
                    .id("api-full-warning")
                    .text_size(px(12.0))
                    .child(FULL_WARNING)
                    .test_support(),
            )
        })
        .child(label(ui, "Expires"))
        .child(menu_button(
            "menu-api-token-expires",
            lives,
            life_key,
            false,
            pick_life,
        ))
        .when(life == Life::Date, |el| el.child(Input::new(date)))
        .child(label(ui, "Workspaces"))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_x(px(14.0))
                .gap_y(px(4.0))
                .child(every)
                .children(boxes),
        )
}

/// New token…: a name, what it can do, when it expires and which workspaces; Make token, then the
/// token once.
fn token_dialog(ws: &Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let ui = ws.ui;
    let entity = cx.entity();
    let name =
        cx.new(|cx| InputState::new(window, cx).placeholder("What it is for: phone, dashboard"));
    let date = cx.new(|cx| InputState::new(window, cx).placeholder("YYYY-MM-DD"));
    let draft = cx.new(|_| Draft {
        scope: Scope::Control,
        life: Life::Days(30),
        workspaces: None,
        address: None,
    });
    let spaces = spaces(ws);
    let first = name.clone();
    window.open_alert_dialog(cx, move |alert, _, cx| {
        let fields = token_fields(&draft, &entity, &spaces, &date, ui, cx);
        let make = {
            let (entity, name, date, draft) =
                (entity.clone(), name.clone(), date.clone(), draft.clone());
            move |window: &mut Window, cx: &mut App| -> bool {
                let (name, date) = (
                    name.read(cx).value().to_string(),
                    date.read(cx).value().to_string(),
                );
                let d = draft.read(cx);
                let (life, scope, workspaces) = (d.life, d.scope, d.workspaces.clone());
                let made = expiry(life, &date).and_then(|expires| {
                    entity.update(cx, |ws, cx| {
                        ws.mint_token(&name, scope, expires, workspaces, cx)
                    })
                });
                match made {
                    Ok(()) => {
                        let e = entity.clone();
                        window.defer(cx, move |window, cx| {
                            e.update(cx, |ws, cx| shown_dialog(ws, window, cx))
                        });
                        true
                    }
                    Err(problem) => {
                        show_notification(window, cx, Notification::warning(problem));
                        false
                    }
                }
            }
        };
        alert
            .confirm()
            .title("New token")
            .w(px(480.0))
            .button_props(DialogButtonProps::default().ok_text("Make token"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(label(ui, "Name"))
                    .child(Input::new(&name))
                    .child(fields),
            )
            .on_ok(move |_, window, cx| make(window, cx))
    });
    first.update(cx, |input, cx| input.focus(window, cx));
}

/// How Pair a phone names an address: Tailscale's reaches the phone anywhere, a home network's
/// only there.
fn address_words(address: Ipv4Addr) -> String {
    if aegis_api::tls::is_tailscale(address) {
        format!("{address} · Tailscale, at home and away")
    } else {
        format!("{address} · this network only")
    }
}

/// Pair a phone…: the token's name, the address the phone reaches this computer at and the
/// token's choices; Show QR codes makes its one-time code and shows the two QR codes. With the
/// API not ready for a phone, Turn it on first.
fn pair_dialog(ws: &Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let entity = cx.entity();
    if let Some(problem) = ws.pairing_problem() {
        if problem == PAIRING_NEEDS {
            prompt::confirm(
                "Pair a phone",
                PAIRING_NEEDS,
                "Turn it on",
                move |window, cx| {
                    entity.update(cx, |ws, cx| ws.fit_api(Way::Pinned, cx));
                    let e = entity.clone();
                    window.defer(cx, move |window, cx| {
                        e.update(cx, |ws, cx| pair_dialog(ws, window, cx))
                    });
                },
                window,
                cx,
            );
        } else {
            show_notification(window, cx, Notification::warning(problem));
        }
        return;
    }
    let addresses = ws.pairing_addresses();
    let Some(&usual) = addresses.first() else {
        show_notification(window, cx, Notification::warning(NO_ADDRESS));
        return;
    };
    let ui = ws.ui;
    let name = cx.new(|cx| InputState::new(window, cx).default_value("Phone"));
    let date = cx.new(|cx| InputState::new(window, cx).placeholder("YYYY-MM-DD"));
    let draft = cx.new(|_| Draft {
        scope: Scope::Control,
        life: Life::Days(90),
        workspaces: None,
        address: Some(usual),
    });
    let spaces = spaces(ws);
    let first = name.clone();
    window.open_alert_dialog(cx, move |alert, _, cx| {
        let chosen = draft.read(cx).address.unwrap_or(usual);
        let options: Vec<(SharedString, SharedString)> = addresses
            .iter()
            .map(|address| (address.to_string().into(), address_words(*address).into()))
            .collect();
        let pick_address = {
            let (draft, entity) = (draft.clone(), entity.clone());
            move |value: SharedString, cx: &mut App| {
                if let Ok(address) = value.parse::<Ipv4Addr>() {
                    draft.update(cx, |d, _| d.address = Some(address));
                    entity.update(cx, |_, cx| cx.notify());
                }
            }
        };
        let fields = token_fields(&draft, &entity, &spaces, &date, ui, cx);
        let show = {
            let (entity, name, date, draft) =
                (entity.clone(), name.clone(), date.clone(), draft.clone());
            move |window: &mut Window, cx: &mut App| -> bool {
                let (name, date) = (
                    name.read(cx).value().to_string(),
                    date.read(cx).value().to_string(),
                );
                let d = draft.read(cx);
                let (life, scope, workspaces) = (d.life, d.scope, d.workspaces.clone());
                let address = d.address.unwrap_or(usual);
                let started = expiry(life, &date).and_then(|expires| {
                    entity.update(cx, |ws, cx| {
                        ws.start_pairing(&name, address, scope, expires, workspaces, cx)
                    })
                });
                match started {
                    Ok(()) => {
                        let e = entity.clone();
                        window.defer(cx, move |window, cx| {
                            e.update(cx, |ws, cx| qr_dialog(ws, window, cx))
                        });
                        true
                    }
                    Err(problem) => {
                        show_notification(window, cx, Notification::warning(problem));
                        false
                    }
                }
            }
        };
        alert
            .confirm()
            .title("Pair a phone")
            .w(px(480.0))
            .button_props(DialogButtonProps::default().ok_text("Show QR codes"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(label(ui, "Name"))
                    .child(Input::new(&name))
                    .child(label(ui, "The phone reaches this computer at"))
                    .child(menu_button(
                        "menu-api-pair-address",
                        options,
                        chosen.to_string().into(),
                        false,
                        pick_address,
                    ))
                    .child(fields),
            )
            .on_ok(move |_, window, cx| show(window, cx))
    });
    first.update(cx, |input, cx| input.focus(window, cx));
}

/// A module of the QR codes, in pixels, and the blank margin around them, in modules (the
/// standard's quiet zone).
const MODULE: f32 = 4.0;
const QUIET: i32 = 4;

/// A QR code's dark modules, as runs along its rows: (row, first column, length).
struct Qr {
    modules: i32,
    runs: Rc<Vec<(i32, i32, i32)>>,
}

impl Qr {
    /// None when `text` is too long for a QR code.
    fn of(text: &str) -> Option<Qr> {
        let code = qrcodegen::QrCode::encode_text(text, qrcodegen::QrCodeEcc::Medium).ok()?;
        let modules = code.size();
        let mut runs = Vec::new();
        for y in 0..modules {
            let mut x = 0;
            while x < modules {
                let start = x;
                while x < modules && code.get_module(x, y) {
                    x += 1;
                }
                if x > start {
                    runs.push((y, start, x - start));
                } else {
                    x += 1;
                }
            }
        }
        Some(Qr {
            modules,
            runs: Rc::new(runs),
        })
    }

    /// Black on white, with its quiet zone.
    fn element(&self, id: &'static str) -> gpui_kit::Stateful<gpui_kit::Div> {
        let side = px((self.modules + 2 * QUIET) as f32 * MODULE);
        let runs = self.runs.clone();
        div()
            .id(id)
            .flex_none()
            .w(side)
            .h(side)
            .bg(gpui_kit::white())
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let black = gpui_kit::hsla(0.0, 0.0, 0.0, 1.0);
                        for &(y, x, length) in runs.iter() {
                            let at = point(
                                bounds.origin.x + px((x + QUIET) as f32 * MODULE),
                                bounds.origin.y + px((y + QUIET) as f32 * MODULE),
                            );
                            let run = size(px(length as f32 * MODULE), px(MODULE));
                            window.paint_quad(fill(Bounds::new(at, run), black));
                        }
                    },
                )
                .size_full(),
            )
    }
}

/// A fingerprint in lines of 8 pairs, to read against the page's.
fn fingerprint_lines(fingerprint: &str) -> Vec<String> {
    let pairs: Vec<&str> = fingerprint.split(':').collect();
    pairs.chunks(8).map(|line| line.join(":")).collect()
}

/// Under the QR codes: whether the phone came for its token yet.
fn paired_words(paired: bool) -> &'static str {
    if paired {
        "Paired: the phone has its token, in Tokens above. You can close this."
    } else {
        "Waiting for the phone. The second code works once, within 10 minutes, while this is open."
    }
}

/// Pair a phone's two QR codes: the page that installs Aegis's authority, then Aegis Mobile with
/// the one-time code. Closing it stops the page and forgets the code.
fn qr_dialog(ws: &Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(pairing) = ws.api.pairing.as_ref() else {
        return;
    };
    let ui = ws.ui;
    let entity = cx.entity();
    let page = pairing.page_url();
    let (page_qr, app_qr) = (Qr::of(&page), Qr::of(&pairing.app_url()));
    let lines = fingerprint_lines(ws.api.fingerprint.as_deref().unwrap_or_default());
    window.open_alert_dialog(cx, move |alert, _, cx| {
        let paired = entity
            .read(cx)
            .api
            .pairing
            .as_ref()
            .is_some_and(|pairing| pairing.paired.is_some());
        let small = |text: SharedString| {
            div()
                .text_size(px(12.0))
                .text_color(ui.title_dim)
                .child(text)
        };
        let step = |title: &'static str| {
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .w(px(220.0))
                .child(div().text_size(px(13.0)).child(title))
        };
        let first = step("1. Install Aegis's authority")
            .children(
                page_qr
                    .as_ref()
                    .map(|qr| qr.element("api-pair-page-qr").test_support()),
            )
            .child(small(format!("Or open {page}").into()))
            .child(small(
                "Then check that the page shows this fingerprint:".into(),
            ))
            .child(
                div()
                    .id("api-pair-fingerprint")
                    .flex()
                    .flex_col()
                    .text_size(px(11.0))
                    .children(lines.iter().map(|line| div().child(line.clone())))
                    .test_support(),
            );
        let second = step("2. Open Aegis Mobile")
            .children(
                app_qr
                    .as_ref()
                    .map(|qr| qr.element("api-pair-app-qr").test_support()),
            )
            .child(small(
                "Scan it once the authority is installed: the phone gets its token and keeps it."
                    .into(),
            ));
        let done = entity.clone();
        alert
            .confirm()
            .title("Pair a phone")
            .w(px(540.0))
            .show_cancel(false)
            .button_props(DialogButtonProps::default().ok_text("Done"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .child(div().flex().gap(px(28.0)).child(first).child(second))
                    .child(
                        div()
                            .id("api-pair-status")
                            .text_size(px(12.0))
                            .child(paired_words(paired))
                            .test_support(),
                    ),
            )
            .on_ok(|_, _, _| true)
            .on_close(move |_, _, cx| done.update(cx, |ws, cx| ws.stop_pairing(cx)))
    });
}

/// The token just made, once, with Copy: its text leaves memory when the dialog closes.
fn shown_dialog(ws: &Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(text) = ws.api.shown.clone() else {
        return;
    };
    let ui = ws.ui;
    let entity = cx.entity();
    window.open_alert_dialog(cx, move |alert, _, _| {
        let done = entity.clone();
        alert
            .confirm()
            .title("Your token")
            .w(px(520.0))
            .show_cancel(false)
            .button_props(DialogButtonProps::default().ok_text("Done"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .id("api-token-text")
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .text_size(px(12.0))
                                    .overflow_hidden()
                                    .child(text.clone())
                                    .test_support(),
                            )
                            .child(Clipboard::new("api-copy-token").value(text.clone())),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(ui.title_dim)
                            .child(SHOWN_ONCE),
                    ),
            )
            .on_ok(|_, _, _| true)
            .on_close(move |_, _, cx| done.update(cx, |ws, _| ws.token_seen()))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_use_the_largest_unit() {
        for (secs, want) in [
            (5, "5s"),
            (59, "59s"),
            (60, "1 min"),
            (3 * 60 + 30, "3 min"),
            (7200, "2 h"),
            (86_400, "1 day"),
            (29 * 86_400, "29 days"),
        ] {
            assert_eq!(span(secs), want, "{secs}");
        }
    }

    #[test]
    fn a_token_says_when_it_ends_and_when_it_was_used() {
        assert_eq!(expiry_words(None, 100), "never expires");
        assert_eq!(
            expiry_words(Some(100 + 29 * 86_400), 100),
            "expires in 29 days"
        );
        assert_eq!(
            expiry_words(Some(100), 100 + 2 * 86_400),
            "expired 2 days ago"
        );
        assert_eq!(used_words(None, 5), "never used");
        let used = Use {
            at: 1000,
            addr: "203.0.113.7".into(),
        };
        assert_eq!(
            used_words(Some(&used), 1000 + 180),
            "last used 3 min ago from 203.0.113.7"
        );
    }

    #[test]
    fn a_shares_line_names_its_workspace_or_says_it_is_gone() {
        use aegis_api::store::{Share, Store};
        let mut store = Store::default();
        let share = Share {
            workspace: "w3".into(),
            way: Way::Public,
            address: "https://t.example.com".into(),
        };
        store
            .mint_share(share, "Another computer", Scope::Full, None, 0)
            .unwrap();
        store.mint("phone", Scope::Read, None, None, 0).unwrap();
        let names = HashMap::from([("w3".to_string(), "API".to_string())]);
        assert_eq!(
            share_line(&store.tokens[0], &names).as_deref(),
            Some("share of API")
        );
        assert_eq!(
            share_line(&store.tokens[0], &HashMap::new()).as_deref(),
            Some("share of a deleted workspace")
        );
        assert_eq!(share_line(&store.tokens[1], &names), None);
        // The workspace it was made for is gone and its id is another workspace's now.
        store.tokens[0].workspaces = Some(vec![]);
        assert_eq!(
            share_line(&store.tokens[0], &names).as_deref(),
            Some("share of a deleted workspace")
        );
    }

    #[test]
    fn new_certificate_counts_the_direct_mirrors() {
        assert_eq!(
            renew_words(0),
            "Clients that pinned or installed the old one stop connecting."
        );
        assert_eq!(
            renew_words(1),
            "Clients that pinned or installed the old one stop connecting, 1 mirror connected \
             directly among them: share again for it."
        );
        assert_eq!(
            renew_words(2),
            "Clients that pinned or installed the old one stop connecting, 2 mirrors connected \
             directly among them: share again for them."
        );
    }

    #[test]
    fn a_date_ends_at_the_end_of_that_day() {
        let end = end_of("2030-01-15").unwrap();
        let next = end_of("2030-01-16").unwrap();
        assert_eq!(
            next - end,
            86_400,
            "a day apart (no change of clock that week)"
        );
        let local = chrono::DateTime::from_timestamp(end as i64, 0)
            .unwrap()
            .with_timezone(&Local);
        assert_eq!(
            local.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2030-01-15 23:59:59"
        );
        for bad in ["", "tomorrow", "2030-13-01", "15/01/2030"] {
            assert_eq!(end_of(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_qr_codes_runs_paint_exactly_its_dark_modules() {
        let text = "https://100.101.102.103:7411/mobile/#code=abcdefghijklmnopqrstuvwxyz012345";
        let code = qrcodegen::QrCode::encode_text(text, qrcodegen::QrCodeEcc::Medium).unwrap();
        let qr = Qr::of(text).unwrap();
        assert_eq!(qr.modules, code.size());
        let mut painted = vec![vec![false; code.size() as usize]; code.size() as usize];
        for &(y, x, length) in qr.runs.iter() {
            assert!(length > 0);
            for column in x..x + length {
                let cell = &mut painted[y as usize][column as usize];
                assert!(!*cell, "painted twice: {column},{y}");
                *cell = true;
            }
        }
        for y in 0..code.size() {
            for x in 0..code.size() {
                assert_eq!(
                    painted[y as usize][x as usize],
                    code.get_module(x, y),
                    "{x},{y}"
                );
            }
        }
        // Runs, not modules: a row's neighbours are one quad.
        let dark = (0..code.size())
            .flat_map(|y| (0..code.size()).map(move |x| (x, y)))
            .filter(|&(x, y)| code.get_module(x, y))
            .count();
        assert!(qr.runs.len() < dark);
        assert!(
            Qr::of(&"x".repeat(8000)).is_none(),
            "too long for any QR code"
        );
    }

    #[test]
    fn the_fingerprint_reads_in_four_lines_of_eight() {
        let fingerprint: Vec<String> = (0u8..32).map(|byte| format!("{byte:02X}")).collect();
        let lines = fingerprint_lines(&fingerprint.join(":"));
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "00:01:02:03:04:05:06:07");
        assert_eq!(lines[3], "18:19:1A:1B:1C:1D:1E:1F");
    }

    #[test]
    fn pair_a_phone_says_which_address_reaches_it_away_from_home() {
        assert_eq!(
            address_words(Ipv4Addr::new(100, 101, 102, 103)),
            "100.101.102.103 · Tailscale, at home and away"
        );
        assert_eq!(
            address_words(Ipv4Addr::new(192, 168, 1, 5)),
            "192.168.1.5 · this network only"
        );
    }

    #[test]
    fn a_tokens_life_becomes_its_expiry() {
        assert_eq!(expiry(Life::Never, ""), Ok(None));
        let before = now();
        let ninety = expiry(Life::Days(90), "").unwrap().unwrap();
        assert!((before + 90 * 86_400..=now() + 90 * 86_400).contains(&ninety));
        assert_eq!(expiry(Life::Date, "2030-01-15"), Ok(end_of("2030-01-15")));
        assert_eq!(
            expiry(Life::Date, "soon"),
            Err("Write the date as YYYY-MM-DD.".to_string())
        );
    }
}
