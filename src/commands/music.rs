use std::{
    sync::{
        Arc,
    },
};

use serenity::{
    async_trait,
    client::{Context},
    framework::{
        standard::{
            macros::{command, group},
            Args,
            CommandResult,
        },
    },
    http::Http,
    model::{channel::Message, prelude::{ChannelId, GuildId}},
    prelude::{Mentionable}, utils::Colour,
};

use songbird::{
    input::Restartable,
    Event,
    EventContext,
    EventHandler as VoiceEventHandler,
    TrackEvent, tracks::PlayMode,
};

use crate::utils::utilities::duration_formatter;


#[group]
#[commands(join, leave, pause, resume, play, stop, queue, skip, resume)]
struct Music;


struct TrackEndNotifier {
    chan_id: ChannelId,
    http: Arc<Http>,
    guild: GuildId,
}

#[async_trait]
impl VoiceEventHandler for TrackEndNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(&[(_state, track)]) = ctx {
            let manager = songbird::Songbird::serenity().get(self.guild);

            if manager.is_none() {
                self.chan_id.send_message(self.http.clone(), |m|
                m.embed(|e|
                    e.title("Queue has Ended")
                    .description(format!("Last song played: **{:?}**\nTo continue listening, play another song!", track.metadata().title.clone().unwrap()))
                    .color(Colour::GOLD)
                )).await;

                return None;
            }

            let manager = manager.unwrap();
            let handler = manager.lock().await;
            let queue = handler.queue().current_queue();

            if queue.len() == 0 {
                self.chan_id.send_message(self.http.clone(), |m|
                    m.embed(|e|
                        e.title("Queue has Ended")
                        .description(format!("Last song played: **{:?}**\nTo continue listening, play another song!", track.metadata().title.clone().unwrap()))
                        .color(Colour::GOLD)
                    )).await;

                return None;
            }

            let next_track = &queue[1];
            let metadata = next_track.metadata();


            self.chan_id.send_message(self.http.clone(), |m|
                m.embed(|e| {
                    e.title("**Now playing**")
                        .url(&metadata.source_url.clone().unwrap() as &str)
                        .description(format!("```\n{}\n```", metadata.title.clone().unwrap()))
                        .color(Colour::DARK_GREEN)
                        .field(
                            "• Duration", 
                            duration_formatter(metadata.duration.unwrap()), 
                            true)
                        .field(
                            "• Author", 
                            &metadata.channel.clone().unwrap() as &str, 
                            true)
                        .field("• URL", 
                        format!("[Click]({})", metadata.source_url.clone().unwrap()), 
                        true);

                        match metadata.thumbnail.clone() {
                            Some(t) => e.thumbnail(t),
                            _ => e
                        }
                }
            )).await;

        }

        None
    }
}


#[command]
#[only_in(guilds)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            msg.reply(ctx, "Not in a voice channel").await;

            return Ok(());
        },
    };

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let (handle_lock, success) = manager.join(guild_id, connect_to).await;

    if let Ok(_channel) = success {
            msg.channel_id
                .say(&ctx.http, &format!("Joined {}", connect_to.mention()))
                .await;

        let chan_id = msg.channel_id;

        let send_http = ctx.http.clone();

        let mut handle = handle_lock.lock().await;

        handle.add_global_event(
            Event::Track(TrackEvent::End),
            TrackEndNotifier {
                chan_id,
                http: send_http.clone(),
                guild: guild_id
            },
        );

    } else {
            msg.channel_id
                .say(&ctx.http, "Error joining the channel")
                .await;
    }

    Ok(())
}


#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    let handler_lock = manager.get(guild_id);
    let has_handler = handler_lock.is_some();
    let handler_lock = handler_lock.unwrap();
    let handler = handler_lock.lock().await;
    let channel = handler.current_channel().unwrap();
    drop(handler);

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await;
        } else {
            msg.channel_id.send_message(ctx, |m|
                m.content(format!("Successfully left <#{}>", channel.0))
            ).await;
        }
        
} else {
        msg.reply(ctx, "Not in a voice channel").await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn pause(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            msg.channel_id.send_message(ctx, |m|
                m.content("Not in a voice channel")
            ).await;

            return Ok(());
        },
    };

    let mut handler = handler_lock.lock().await;

    if handler.queue().current().is_none() {
        msg.channel_id.send_message(ctx, |m|
            m.content("Nothing playing currently")
        ).await;
        return Ok(());
    }

    let current_track = handler.queue()
        .current()
        .unwrap();

    let play_status = current_track.get_info()
        .await
        .unwrap()
        .playing;
        

    if matches!(play_status, PlayMode::Play) {
        msg.channel_id.say(&ctx.http, "Already paused").await;
    } else {
        current_track.pause();

        msg.channel_id.say(&ctx.http, format!("Paused **{}**", current_track.metadata().title.clone().unwrap())).await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let search = args.message().to_string();

    if search.len().eq(&0) {
        resume(ctx, msg, args.clone()).await.unwrap();
        return Ok(())
    }

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let None = manager.get(guild_id) {
        join(ctx, msg, args.clone()).await.unwrap();
        if let None = manager.get(guild_id) {
            
            return Ok(());
        }
    }

    let handler_lock = manager.get(guild_id).unwrap();
    let mut handler = handler_lock.lock().await;
    let queue = handler.queue();

    let source: Restartable;
    if search.starts_with("http") {
        source = Restartable::ytdl(search.clone(), true).await.unwrap();
    } else {
        source = Restartable::ytdl_search(&search, true).await.unwrap();
    }
    
    if queue.current().is_none() {
        let song = handler.enqueue_source(source.into());
        drop(handler);
        let metadata = song.metadata();

        msg.channel_id.send_message(ctx.clone(), |m|
            m.embed(|e| {
                e.title("**Now playing**")
                    .url(&metadata.source_url.clone().unwrap() as &str)
                    .description(format!("```\n{}\n```", metadata.title.clone().unwrap()))
                    .color(Colour::DARK_GREEN)
                    .field(
                        "• Duration", 
                        duration_formatter(metadata.duration.unwrap()), 
                        true)
                    .field(
                        "• Requested by",
                        msg.author.mention(),
                        true
                    )
                    .field(
                        "• Author", 
                        &metadata.channel.clone().unwrap() as &str, 
                        true)
                    .field("• URL", 
                    format!("[Click]({})", metadata.source_url.clone().unwrap()), 
                    true);

                    match metadata.thumbnail.clone() {
                        Some(t) => e.thumbnail(t),
                        _ => e
                    }
            }
        )).await.unwrap();

    } else {
        let track = handler.enqueue_source(source.into());
        drop(handler);
        let metadata = track.metadata();

        msg.channel_id.send_message(ctx, |m| 
            m.content(format!("Enqueued **{}** by **{}**", metadata.title.clone().unwrap(), metadata.channel.clone().unwrap()))
            ).await.unwrap();
    }

    Ok(())
}


#[command]
#[only_in(guilds)]
async fn queue(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let mut description: String = String::new();
        let now_playing = handler.queue().current();
        let queue = handler.queue().current_queue();
        drop(handler);

        if queue.len().eq(&0) || now_playing.is_none() {
            msg.channel_id.send_message(ctx, |m| m.content("Nothing in the queue")).await;

            return Ok(())
        } 

        let now_playing = now_playing.unwrap();
        let metadata = now_playing.metadata();

        for (i, j) in queue.iter().zip(
            1..(queue.len() + 1)
        ) {
            if j == 1 { continue }
            description.push_str(
                &format!("**{})** [{}]({})\n", j-1, i.metadata().title.clone().unwrap(), i.metadata().source_url.clone().unwrap()) as &str
            )
        }

        msg.channel_id.send_message(ctx, |m| 
            m.embed(|e|
                e.field(
                    "• Now Playing",
                    format!("[{}]({})", metadata.title.clone().unwrap(), metadata.source_url.clone().unwrap()),
                    false
                )
                .field(
                    "• Up Next",
                    description.as_str(),
                    false
                )
                .color(Colour::GOLD)
            )).await;


    } else {
        msg.channel_id
            .say(&ctx.http, "Not playing any music right now")
            .await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn skip(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let _ = queue.skip();

        msg.channel_id
            .say(
                &ctx.http,
                format!("Song skipped: {} in queue.", queue.len()),
            )
            .await;
    } else {
        msg.channel_id
            .say(&ctx.http, "Not in a voice channel to play in")
            .await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn stop(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        queue.stop();

        msg.channel_id.say(&ctx.http, "Skipped song and cleared queue").await;
    } else {
        msg.channel_id
            .say(&ctx.http, "Not in a voice channel to play in")
            .await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn resume(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        let mut handler = handler_lock.lock().await;

        if handler.queue().current().is_none() {
            msg.channel_id.send_message(ctx, |m|
                m.content("Nothing playing currently")
            ).await;
            return Ok(());
        }
    
        let current_track = handler.queue()
            .current()
            .unwrap();
    
        let play_status = current_track.get_info()
            .await
            .unwrap()
            .playing;

        if matches!(play_status, PlayMode::Pause) {
            current_track.play();
            msg.channel_id.send_message(ctx, |m|
                m.content(format!("Resumed **{}**", current_track.metadata().title.clone().unwrap()))
            ).await;
        }

    } else {
        msg.channel_id
            .say(&ctx.http, "No music to resume")
            .await;
    }

    Ok(())
}
