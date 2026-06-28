//! Notification tools (mirrors `handlers::notification`). All require an authenticated user.

use crate::mcp::auth::require_user;
use crate::mcp::support::{json_ok, map_core, mk_tool, parse_args, run};
use hq_core::Service;
use hq_types::hq::NotificationId;
use mcpkit::server::capability::tools::ToolService;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Deserialize)]
struct NotificationRef {
    id: NotificationId,
}

pub fn register(tools: &mut ToolService, service: &Arc<Service>) {
    let svc = service.clone();
    tools.register(
        mk_tool(
            "list_notifications",
            "List the current user's notifications.",
            json!({"type": "object"}),
        ),
        move |_args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let notifications = svc.notification.list_by_user(uid).await.map_err(map_core)?;
                json_ok(&notifications)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_unread_notification_count",
            "Get the current user's unread notification count.",
            json!({"type": "object"}),
        ),
        move |_args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let count = svc
                    .notification
                    .get_unread_count(uid)
                    .await
                    .map_err(map_core)?;
                json_ok(&count)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "mark_notification_read",
            "Mark a notification as read.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let NotificationRef { id } = parse_args(args)?;
                let notification = svc
                    .notification
                    .mark_as_read(id, uid)
                    .await
                    .map_err(map_core)?;
                json_ok(&notification)
            })
        },
    );
}
