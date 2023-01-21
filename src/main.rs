mod commands;
mod events;
mod utils;

use std::collections::HashSet;
use std::{env, fs};

use serenity::async_trait;
use serenity::framework::StandardFramework;
use serenity::model::gateway::Ready;
use serenity::model::prelude::{UserId, Activity};
use serenity::prelude::*;

use songbird::SerenityInit;

struct Events;

#[async_trait]
impl EventHandler for Events {
    async fn ready(&self, ctx: Context, ready: Ready) {
        ctx.set_activity(Activity::watching("OSD")).await;
        ctx.idle().await;

        println!("{} is ready", ready.user.name);
    }
}


#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Read the json data
    let data: String = fs::read_to_string("config.json")
        .expect("No config.json file in folder");
    
    let cfg: serde_json::Value = serde_json::from_str(&data)
        .expect("config.json file is not in proper json format");
    
    let mut token: String = cfg["token"].clone().to_string().replace("\"", "");

    // Check command arguments to see if test running
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|i| i == "t") {
        token = cfg["test-token"].clone().to_string().replace("\"", "");
    }

    // Define used intents
    let intents = GatewayIntents::all();

    // Define owners
    let owners: HashSet<UserId> = HashSet::from([UserId::from(667760867483582492), UserId::from(732627312805412884)]);

    // Define framework
    let framework = StandardFramework::new()
        .configure(|c| c
            .on_mention(Some(UserId::from(796491764092633128)))
            .prefix("!")
            .with_whitespace(false)
            .owners(owners)
            .case_insensitivity(true)
            .no_dm_prefix(true)
        )
        .help(&commands::help::HELP) // src/commands/help.rs
        .before(events::command_events::before) // src/events/command_events.rs
        .after(events::command_events::after) // src/events/command_events.rs
        .group(&commands::utility::UTILITIES_GROUP) // src/commands/util.rs
        .group(&commands::music::MUSIC_GROUP); // src/commands/music.rs

    // Initialize client
    let mut client = 
        Client::builder(&token, intents)
            .event_handler(Events)
            .framework(framework)
            .register_songbird()
            .await
            .expect("Error creating client");

    // Start the client with error handler
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}