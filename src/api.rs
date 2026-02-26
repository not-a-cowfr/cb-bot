use std::time::Duration;

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use reqwest::Response;
use serenity::all::{
	ButtonStyle,
	ChannelId,
	CreateAttachment,
	CreateButton,
	CreateEmbed,
	CreateInteractionResponse,
	CreateInteractionResponseMessage,
	CreateMessage,
	EditMessage,
	RoleId,
};

use crate::AppState;
use crate::types::Error;

pub async fn handle_request(
	State(app_state): State<AppState>,
	mut mp: Multipart,
) -> Result<StatusCode, StatusCode> {
	let field = loop {
		match mp.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
			| Some(f)
				if f.content_type()
					.map_or(false, |ct| ct.starts_with("image/")) =>
			{
				break f;
			},
			| Some(_) => continue,
			| None => return Err(StatusCode::BAD_REQUEST),
		}
	};

	let filename = field.file_name().unwrap_or("image.png").to_string();
	let content_type = field.content_type().unwrap_or("image/png").to_string();
	let bytes = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;

	let channel_id = ChannelId::new(
		std::env::var("CHANNEL")
			.expect("Missing CHANNEL env var")
			.parse::<u64>()
			.expect("CHANNEL env var is not a valid channel id"),
	);
	let role_id = RoleId::new(
		std::env::var("VERIFYER_ROLE")
			.expect("Missing VERIFYER_ROLE env var")
			.parse::<u64>()
			.expect("VERIFYER_ROLE env var is not a valid role id"),
	);

	let mut m = channel_id
		.send_message(
			&app_state.http,
			CreateMessage::default()
				.content(format!("<@&{role_id}>"))
				.add_file(CreateAttachment::bytes(bytes, &filename))
				.embed(
					CreateEmbed::default()
						.title("New Image Upload")
						.image(format!("attachment://{filename}")),
				)
				.button(
					CreateButton::new("accept")
						.style(ButtonStyle::Success)
						.label("Accept"),
				)
				.button(
					CreateButton::new("decline")
						.style(ButtonStyle::Danger)
						.label("Decline"),
				),
		)
		.await
		.map_err(|e| {
			eprintln!("Failed to send message: {e}");
			StatusCode::INTERNAL_SERVER_ERROR
		})?;

	let http = app_state.http.clone();
	let shard = app_state.shard.clone();

	tokio::spawn(async move {
		let interaction = match m
			.await_component_interaction(&shard)
			.timeout(Duration::from_secs(60 * 3))
			.await
		{
			| Some(x) => x,
			| None => {
				m.edit(
					&http,
					EditMessage::default()
						.button(
							CreateButton::new("accept")
								.style(ButtonStyle::Success)
								.label("Accept")
								.disabled(true),
						)
						.button(
							CreateButton::new("decline")
								.style(ButtonStyle::Danger)
								.label("Decline")
								.disabled(true),
						),
				)
				.await
				.ok();

				return Err(StatusCode::INTERNAL_SERVER_ERROR);
			},
		};

		interaction
			.create_response(&http, CreateInteractionResponse::Acknowledge)
			.await
			.map_err(|e| {
				eprintln!("Failed to defer: {e}");
				StatusCode::INTERNAL_SERVER_ERROR
			})?;

		if interaction
			.member
			.clone()
			.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
			.roles
			.contains(&role_id)
		{
			let _ = interaction.create_response(
				&http,
				CreateInteractionResponse::Message(
					CreateInteractionResponseMessage::new()
						.content("You do not have the correct role to use this!")
						.ephemeral(true),
				),
			);
		}

		let label = match interaction.data.custom_id.as_str() {
			| "accept" => {
				let image_url = m
					.embeds
					.first()
					.map(|e| e.image.clone().unwrap().url)
					.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

				let res = upload_image(image_url, filename, content_type)
					.await
					.map_err(|e| {
						eprintln!("Error uploading image: {}", e);
						StatusCode::INTERNAL_SERVER_ERROR
					})?;

				if !res.status().is_success() {
					eprintln!("Upload returned {}", res.status());
					return Err(StatusCode::INTERNAL_SERVER_ERROR);
				}

				"✅ Accepted"
			},
			| "decline" => "❌ Declined",
			| _ => return Err(StatusCode::BAD_REQUEST),
		};
		interaction
			.edit_response(
				&http,
				serenity::all::EditInteractionResponse::default()
					.content(label)
					.button(
						CreateButton::new("accept")
							.style(ButtonStyle::Success)
							.label("Accept")
							.disabled(true),
					)
					.button(
						CreateButton::new("decline")
							.style(ButtonStyle::Danger)
							.label("Decline")
							.disabled(true),
					),
			)
			.await
			.map_err(|e| {
				eprintln!("Failed to update message: {e}");
				StatusCode::INTERNAL_SERVER_ERROR
			})?;

		Ok(StatusCode::ACCEPTED)
	});

	Ok(StatusCode::ACCEPTED)
}

pub async fn upload_image(
	image_url: String,
	file_name: String,
	content_type: String,
) -> Result<Response, Error> {
	let img_bytes = reqwest::get(&image_url).await?.bytes().await?;

	let part = reqwest::multipart::Part::bytes(img_bytes.to_vec())
		.file_name(file_name.clone())
		.mime_str(&content_type)?;

	let form = reqwest::multipart::Form::new().part("files[]", part);

	let api_key = std::env::var("API_KEY")?;

	let client = reqwest::Client::new();

	let url = format!("https://cuteboys.love/api/upload?key={}", api_key);

	let res = client.post(url).multipart(form).send().await?;

	Ok(res)
}
