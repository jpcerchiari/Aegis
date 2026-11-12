//! Idle sleep follows local work, even when the window or its workspace is hidden.

use serde_json::json;
use std::time::Duration;

use super::agents_view::agent;
use super::subagents::hook;
use super::*;

#[gpui_kit::test]
fn sleep_is_prevented_until_an_agents_turn_ends(cx: &mut TestAppContext) {
    let (handle, ws) = open(cx);
    let (_, terminal) = agent(handle, &ws, cx, 1);
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    for _ in 0..2 {
        hook(
            handle,
            &ws,
            cx,
            &terminal,
            json!({"hook_event_name": "UserPromptSubmit"}),
        );
        cx.run_until_parked();
        assert_eq!(cx.active_idle_sleep_preventions(), 1);
    }
    let power = ws.read_with(cx, |ws, cx| {
        serde_json::to_value(ws.api_state(cx)).unwrap()["power"].clone()
    });
    assert_eq!(
        power,
        json!({"requested": true, "preventing_idle_sleep": true, "error": null})
    );
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "PermissionRequest"}),
    );
    cx.run_until_parked();
    assert_eq!(
        cx.active_idle_sleep_preventions(),
        1,
        "the phone can still answer the approval"
    );
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "PostToolUse"}),
    );
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "Stop"}),
    );
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[gpui_kit::test]
fn work_in_a_hidden_workspace_keeps_sleep_prevented_until_that_workspace_stops(
    cx: &mut TestAppContext,
) {
    let (handle, ws) = open(cx);
    let (_, terminal) = agent(handle, &ws, cx, 1);
    let first = ws.read_with(cx, |ws, _| ws.space.id.clone());
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "UserPromptSubmit"}),
    );
    cx.update_window(handle, |_, window, cx| {
        ws.update(cx, |ws, cx| ws.new_space(window, cx))
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    let (_, second) = agent(handle, &ws, cx, 1);
    hook(
        handle,
        &ws,
        cx,
        &second,
        json!({"hook_event_name": "UserPromptSubmit"}),
    );
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "Stop"}),
    );
    cx.run_until_parked();
    assert_eq!(
        cx.active_idle_sleep_preventions(),
        1,
        "the other agent still works"
    );
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "UserPromptSubmit"}),
    );
    hook(handle, &ws, cx, &second, json!({"hook_event_name": "Stop"}));
    cx.update_window(handle, |_, window, cx| {
        ws.update(cx, |ws, cx| ws.stop_now(&first, window, cx))
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[gpui_kit::test]
fn a_late_sleep_guard_is_released_when_the_turn_already_ended(cx: &mut TestAppContext) {
    let (handle, ws) = open(cx);
    let (_, terminal) = agent(handle, &ws, cx, 1);
    cx.set_idle_sleep_prevention_delay(Duration::from_secs(3));
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "UserPromptSubmit"}),
    );
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "Stop"}),
    );
    cx.executor().advance_clock(Duration::from_secs(4));
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    let power = ws.read_with(cx, |ws, cx| {
        serde_json::to_value(ws.api_state(cx)).unwrap()["power"].clone()
    });
    assert_eq!(
        power,
        json!({"requested": false, "preventing_idle_sleep": false, "error": null})
    );
}

#[gpui_kit::test]
fn a_sleep_prevention_failure_is_visible_and_the_next_turn_can_try_again(cx: &mut TestAppContext) {
    let (handle, ws) = open(cx);
    let (_, terminal) = agent(handle, &ws, cx, 1);
    cx.set_idle_sleep_prevention_fails(true);
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "UserPromptSubmit"}),
    );
    cx.run_until_parked();
    let power = ws.read_with(cx, |ws, cx| {
        serde_json::to_value(ws.api_state(cx)).unwrap()["power"].clone()
    });
    assert_eq!(power["requested"], true);
    assert_eq!(power["preventing_idle_sleep"], false);
    assert!(power["error"].as_str().unwrap().contains("set to fail"));
    cx.set_idle_sleep_prevention_fails(false);
    ws.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(
        cx.active_idle_sleep_preventions(),
        0,
        "a failure does not retry on each redraw"
    );
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "Stop"}),
    );
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "UserPromptSubmit"}),
    );
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
}

#[gpui_kit::test]
fn the_aegis_agent_also_prevents_sleep_and_closing_releases_it(cx: &mut TestAppContext) {
    let (handle, ws) = open(cx);
    ws.update(cx, |ws, cx| {
        ws.aegis_agent.conversation.person("Work on this");
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    ws.update(cx, |ws, cx| {
        ws.aegis_agent
            .conversation
            .apply(::aegis_agent::ChatEvent::TurnEnded {
                ms: Some(100),
                cost_usd: None,
                error: None,
            });
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    ws.update(cx, |ws, cx| {
        ws.aegis_agent.conversation.person("Another turn");
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    drop(ws);
    cx.update_window(handle, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[gpui_kit::test]
fn a_remote_agents_work_does_not_hold_this_computer_awake(cx: &mut TestAppContext) {
    let (_handle, ws, _dir) = super::agents_view::mirrored_agent(cx, "working");
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    let power = ws.read_with(cx, |ws, cx| {
        serde_json::to_value(ws.api_state(cx)).unwrap()["power"].clone()
    });
    assert_eq!(power["requested"], false);
}

#[gpui_kit::test]
fn background_subagents_keep_sleep_prevented_while_the_parent_asks(cx: &mut TestAppContext) {
    let (handle, ws) = open(cx);
    let (_, terminal) = agent(handle, &ws, cx, 1);
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({
            "hook_event_name": "SubagentStart", "agent_id": "a1", "agent_type": "Explore"
        }),
    );
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "PermissionRequest"}),
    );
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({
            "hook_event_name": "SubagentStop", "agent_id": "a1", "agent_type": "Explore"
        }),
    );
    cx.run_until_parked();
    assert_eq!(
        cx.active_idle_sleep_preventions(),
        1,
        "the parent still awaits approval"
    );
    hook(
        handle,
        &ws,
        cx,
        &terminal,
        json!({"hook_event_name": "Stop"}),
    );
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}
