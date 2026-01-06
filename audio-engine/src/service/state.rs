use mockall::automock;

use crate::{
    error::ZakoResult,
    types::{GuildId, SessionState},
};

#[automock]
pub trait StateService: Send + Sync + 'static {
    async fn get_session(&self, guild_id: GuildId) -> ZakoResult<Option<SessionState>>;
    async fn save_session(&self, session: &SessionState) -> ZakoResult<()>;
    async fn delete_session(&self, guild_id: GuildId) -> ZakoResult<()>;

    async fn modify_session<F>(&self, guild_id: GuildId, f: F) -> ZakoResult<()>
    where
        F: FnOnce(&mut SessionState) + Send + 'static;
}
