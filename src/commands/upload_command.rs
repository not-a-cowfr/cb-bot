use serenity::all::{Attachment, RoleId};

use crate::api::upload_image;
use crate::types::{Context, Error};

#[poise::command(slash_command, prefix_command)]
pub async fn upload(
	ctx: Context<'_>,
	#[description = "image to upload"] image: Attachment,
) -> Result<(), Error> {
	ctx.defer().await?;

	if !image
		.content_type
		.as_ref()
		.ok_or("Uploaded file is not an image!")?
		.starts_with("image/")
	{
		ctx.reply("Uploaded attatchment is not an image!").await?;
		return Ok(());
	}

	let role_id = RoleId::new(
		std::env::var("VERIFYER_ROLE")
			.expect("Missing VERIFYER_ROLE env var")
			.parse::<u64>()
			.expect("VERIFYER_ROLE env var is not a valid role id"),
	);

	if !ctx
		.author()
		.has_role(
			ctx.http(),
			ctx.guild_id().ok_or("This can only be used in guilds")?,
			role_id,
		)
		.await?
	{
		ctx.reply("You do not have the correct role to use this!")
			.await?;
		return Ok(());
	}

	let res = upload_image(
		image.url,
		image.filename,
		image.content_type.unwrap_or("image/png".to_string()),
	)
	.await?;

	if res.status().is_success() {
		ctx.reply("Uploaded successfully").await?;
	} else {
		ctx.reply(format!("Uploaded failed! {}", res.text().await?))
			.await?;
	}

	Ok(())
}
