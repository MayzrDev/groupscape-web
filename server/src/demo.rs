use crate::error::ApiError;
use std::sync::OnceLock;

/// The public, permanently-seeded demo group (see the `seed` binary). Reachable via a published,
/// non-secret token, so unlike every other group it must never accept writes from that token -
/// see `reject_if_demo` below.
pub const DEMO_GROUP_NAME: &str = "@EXAMPLE";

/// Resolved once at server startup (see `main.rs`) rather than hardcoded as a literal id, since
/// `group_id` is DB-assigned - but it never changes again after that, so every write handler can
/// treat it as a fixed constant for the lifetime of the process.
static DEMO_GROUP_ID: OnceLock<Option<i64>> = OnceLock::new();

/// Called once at startup with the demo group's id (`None` if it hasn't been seeded yet, e.g. a
/// fresh local DB before `cargo run --bin seed` has ever been run).
pub fn init(demo_group_id: Option<i64>) {
    let _ = DEMO_GROUP_ID.set(demo_group_id);
}

fn is_demo_group(group_id: i64) -> bool {
    DEMO_GROUP_ID.get().copied().flatten() == Some(group_id)
}

/// Called at the top of every write handler reachable via a group's shared token (see
/// `authed.rs`) - the demo group's token is public, so it's the one group that must reject
/// mutations regardless of which handler receives them.
pub fn reject_if_demo(group_id: i64) -> Result<(), ApiError> {
    if is_demo_group(group_id) {
        Err(ApiError::DemoGroupReadOnlyError)
    } else {
        Ok(())
    }
}
