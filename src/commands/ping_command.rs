use crate::types::{Context, Error};

#[poise::command(slash_command, prefix_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
	ctx.reply(format!("pong! {}ms", ctx.ping().await.as_millis()))
		.await?;

	Ok(())
}
