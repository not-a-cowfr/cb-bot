use std::env::var;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::routing::post;
use serenity::all::{ClientBuilder, GatewayIntents, Http, ShardMessenger};

use crate::api::handle_request;

mod api;
mod commands;

mod types {
	pub type Error = Box<dyn std::error::Error + Send + Sync>;
	pub type Context<'a> = poise::Context<'a, super::Data, Error>;
}

pub struct Data {}

#[derive(Clone)]
pub struct AppState {
	pub http:  Arc<Http>,
	pub shard: ShardMessenger,
}

#[tokio::main]
async fn main() {
	dotenv::dotenv().ok();

	let options = poise::FrameworkOptions {
		commands: commands::get_all_commands(),
		prefix_options: poise::PrefixFrameworkOptions {
			prefix: Some("!".into()),
			edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
				Duration::from_secs(3600),
			))),
			..Default::default()
		},
		pre_command: |ctx| {
			Box::pin(async move {
				println!("[COMMAND] started {}", ctx.command().qualified_name);
			})
		},
		post_command: |ctx| {
			Box::pin(async move {
				println!("[COMMAND] completed {}", ctx.command().qualified_name);
			})
		},
		skip_checks_for_owners: false,
		..Default::default()
	};

	let framework = poise::Framework::builder()
		.setup(move |ctx, _ready, framework| {
			Box::pin(async move {
				println!("Logged in as {}", _ready.user.name);
				poise::builtins::register_globally(ctx, &framework.options().commands).await?;

				let http = ctx.http.clone();
				let shard = ctx.shard.clone();

				let app_state = AppState {
					http,
					shard,
				};

				let app = Router::new()
					.route("/", post(handle_request))
					.with_state(app_state);

				let listener = tokio::net::TcpListener::bind(format!(
					"0.0.0.0:{}",
					var("API_PORT").unwrap_or("3000".to_string())
				))
				.await
				.unwrap();

				println!("listening on {}", listener.local_addr().unwrap());

				tokio::task::spawn_blocking(move || {
					if let Err(err) = tokio::runtime::Handle::current()
						.block_on(axum::serve(listener, app).into_future())
					{
						eprintln!("api error: {:?}", err);
					}
				});

				Ok(Data {})
			})
		})
		.options(options)
		.build();

	let token = var("DISCORD_BOT_TOKEN")
		.expect("Missing `DISCORD_BOT_TOKEN` env var, please include this in your .env file");
	let intents = GatewayIntents::non_privileged()
		| GatewayIntents::GUILD_MESSAGES
		| GatewayIntents::DIRECT_MESSAGES;

	let mut client = ClientBuilder::new(token, intents)
		.framework(framework)
		.await
		.unwrap();

	client.start().await.unwrap();
}
