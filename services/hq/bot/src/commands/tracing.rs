use poise::BoxFuture;
use tracing::Instrument as _;

type Cmd = poise::Command<crate::Data, crate::Error>;
type SlashFn = for<'a> fn(
    poise::ApplicationContext<'a, crate::Data, crate::Error>,
) -> BoxFuture<'a, Result<(), poise::FrameworkError<'a, crate::Data, crate::Error>>>;

struct OriginalSlashAction(SlashFn);
// SAFETY: fn pointers are always Send + Sync
unsafe impl Send for OriginalSlashAction {}
unsafe impl Sync for OriginalSlashAction {}

fn traced_slash_action(
    ctx: poise::ApplicationContext<'_, crate::Data, crate::Error>,
) -> BoxFuture<'_, Result<(), poise::FrameworkError<'_, crate::Data, crate::Error>>> {
    let original = ctx
        .command
        .custom_data
        .downcast_ref::<OriginalSlashAction>()
        .expect("missing OriginalSlashAction in custom_data")
        .0;

    let span = tracing::info_span!(
        "slash_command",
        command = ctx.command.qualified_name.as_str(),
        user_id = ctx.interaction.user.id.get(),
        guild_id = ctx.interaction.guild_id.map(|g| g.get()),
    );
    Box::pin(original(ctx).instrument(span))
}

/// Wraps a command (and all its subcommands) so their `slash_action` runs inside
/// a `tracing::info_span!("slash_command", command, user_id, guild_id)`.
pub fn with_tracing(mut cmd: Cmd) -> Cmd {
    cmd.subcommands = cmd.subcommands.into_iter().map(with_tracing).collect();

    if let Some(orig) = cmd.slash_action {
        cmd.custom_data = Box::new(OriginalSlashAction(orig));
        cmd.slash_action = Some(traced_slash_action);
    }
    cmd
}
