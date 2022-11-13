use std::time::Instant;

use serenity::framework::standard::{CommandResult};
use serenity::framework::standard::macros::{group, command};
use serenity::model::channel::Message;
use serenity::prelude::*;


#[group]
#[commands(ping)]
struct Utilities;

#[command]
#[description = "Check the latency of the bot"]
#[usage = "!ping"]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    let before = Instant::now();
    let mut m = msg.reply(ctx, "Pong!").await?;
    let after = Instant::now();
    let delay = (after - before).as_millis();

    m.edit(ctx, |c| c.content(format!("Pong! `{}` ms", delay))).await.expect("Error in editing message in ping command");

    Ok(())
}