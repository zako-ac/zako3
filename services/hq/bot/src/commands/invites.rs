use crate::{Context, Error, ui};
use poise::serenity_prelude as serenity;

fn invite_url(client_id: &str, permissions: &str) -> String {
    format!(
        "https://discord.com/oauth2/authorize?client_id={client_id}&permissions={permissions}&integration_type=0&scope=bot+applications.commands"
    )
}

#[poise::command(
    slash_command,
    name_localized("ko", "초대"),
    description_localized("en-US", "Get invite links for the bots"),
    description_localized("ko", "봇 초대 링크들을 가져옵니다")
)]
pub async fn invites(ctx: Context<'_>) -> Result<(), Error> {
    let config = &ctx.data().service.config;

    let mut buttons: Vec<serenity::CreateButton> = Vec::new();

    let main_url = invite_url(&config.discord_client_id, &config.bot_invite_permissions);
    buttons.push(serenity::CreateButton::new_link(main_url).label("메인 봇 초대"));

    for (i, id) in config.sub_bot_ids.iter().enumerate() {
        let url = invite_url(id, &config.bot_invite_permissions);
        buttons
            .push(serenity::CreateButton::new_link(url).label(format!("서브 봇 {} 초대", i + 1)));
    }

    let rows: Vec<serenity::CreateActionRow> = buttons
        .chunks(5)
        .map(|chunk| serenity::CreateActionRow::Buttons(chunk.to_vec()))
        .collect();

    let embed = ui::embeds::web_link_embed("봇 초대", "아래 버튼을 눌러 봇을 서버에 초대하세요.");

    ctx.send(poise::CreateReply::default().embed(embed).components(rows))
        .await?;

    Ok(())
}
