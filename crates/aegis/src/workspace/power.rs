//! Keeps this computer awake while its local agents work, including in hidden workspaces.

use aegis_api::model::PowerView;
use gpui_kit::{ActivityGuard, Context, Task};

use super::Workspace;
use super::agents_view::Doing;

#[derive(Default)]
pub(super) struct Power {
    guard: Option<ActivityGuard>,
    pending: Option<Task<()>>,
    error: Option<String>,
}

impl Workspace {
    fn needs_awake(&self) -> bool {
        self.aegis_agent.conversation.working
            || self
                .spaces()
                .filter(|space| !space.stopped && !space.is_remote())
                .flat_map(|space| space.terminals())
                .any(|(_, pane)| {
                    pane.agent().is_some()
                        && pane.follows.is_none()
                        && (matches!(
                            self.doing(pane),
                            Doing::Working(_) | Doing::AwaitsSubagents(_) | Doing::NeedsYou(_)
                        ) || pane.subagents_at_work() > 0)
                })
    }

    pub(super) fn power_view(&self) -> PowerView {
        PowerView {
            requested: self.power.guard.is_some()
                || self.power.pending.is_some()
                || self.power.error.is_some(),
            preventing_idle_sleep: self.power.guard.is_some(),
            error: self.power.error.clone(),
        }
    }

    pub(super) fn update_power(&mut self, cx: &mut Context<Self>) {
        if !self.needs_awake() {
            let changed = self.power_view() != PowerView::default();
            self.power = Power::default();
            if changed {
                cx.notify();
            }
            return;
        }
        if self.power_view().requested {
            return;
        }
        let acquire = cx.prevent_idle_sleep("An Aegis agent is working");
        self.power.pending = Some(cx.spawn(async move |ws, cx| {
            let result = acquire.await;
            ws.update(cx, |ws, cx| {
                ws.power.pending = None;
                if !ws.needs_awake() {
                    return;
                }
                match result {
                    Ok(guard) => ws.power.guard = Some(guard),
                    Err(error) => {
                        let message = format!("Couldn't prevent idle sleep: {error:#}");
                        eprintln!("aegis: {message}");
                        ws.power.error = Some(message);
                    }
                }
                cx.notify();
            })
            .ok();
        }));
    }
}
