use std::{
    sync::{
        Arc,
    }, fs::File, io::Write,
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
    input::{Restartable, Metadata},
    Event,
    EventContext,
    EventHandler as VoiceEventHandler,
    TrackEvent, tracks::{PlayMode, LoopState, self},
};

use crate::utils::utilities::{duration_formatter, num_prefix};


#[group]
#[commands(join, leave, pause, resume, play, stop, queue, skip, remove, loop_command, volume)]
struct Music;


struct TrackEndNotifier {
    chan_id: ChannelId,
    http: Arc<Http>,
    context: Context,
    guild: GuildId
}

#[async_trait]
impl VoiceEventHandler for TrackEndNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(&[(_state, track)]) = ctx {
            let manager = songbird::get(&self.context).await.unwrap();
            let handle = manager.get(self.guild);

            if handle.is_none() {
                self.chan_id.send_message(self.http.clone(), |m|
                m.embed(|e|
                    e.title("Queue has Ended")
                    .description(format!("Last song played: **{:?}**\nTo continue listening, play another song!", track.metadata().title.clone().unwrap()))
                    .color(Colour::GOLD)
                )).await;

                drop(handle);
                self.context.idle().await;

                return None;
            }

            let handle = handle.unwrap();
            let mut handler = handle.lock().await;
            let queue = handler.queue().current_queue();

            if queue.len() == 0 {
                self.chan_id.send_message(self.http.clone(), |m|
                    m.embed(|e|
                        e.title("Queue has Ended")
                        .description(format!("Last song played: **{:?}**\nTo continue listening, play another song!", track.metadata().title.clone().unwrap()))
                        .color(Colour::GOLD)
                    )).await;

                handler.leave().await;
                drop(handler);

                return None;
            }

            let next_track = &queue[0];
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
#[description = "Joins the voice channel you are currently in"]
#[usage = "!join"]
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
            msg.reply(ctx, "Not in a voice channel").await?;

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
                .await?;

        let chan_id = msg.channel_id;

        let send_http = ctx.http.clone();

        let mut handle = handle_lock.lock().await;

        handle.add_global_event(
            Event::Track(TrackEvent::End),
            TrackEndNotifier {
                chan_id,
                http: send_http.clone(),
                context: ctx.clone(),
                guild: guild_id
            },
        );

        drop(handle);

    } else {
            msg.channel_id
                .say(&ctx.http, "Error joining the channel")
                .await;
    }

    Ok(())
}


#[command]
#[description = "Leaves the voice channel you are currently in"]
#[usage = "!leave"]
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
                    .await?;
        } else {
            msg.channel_id.send_message(ctx, |m|
                m.content(format!("Successfully left <#{}>", channel.0))
            ).await?;
        }
        
} else {
        msg.reply(ctx, "Not in a voice channel").await?;
    }

    Ok(())
}

#[command]
#[description = "Pauses the current song playing"]
#[usage = "!pause"]
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
            ).await?;

            return Ok(());
        },
    };

    let handler = handler_lock.lock().await;

    if handler.queue().current().is_none() {
        msg.channel_id.send_message(ctx, |m|
            m.content("Nothing playing currently")
        ).await?;
        return Ok(());
    }

    let current_track = handler.queue()
        .current()
        .unwrap();

    let play_status = current_track.get_info()
        .await
        .unwrap()
        .playing;

    drop(handler);
        

    if matches!(play_status, PlayMode::Play) {
        msg.channel_id.say(&ctx.http, "Already paused").await?;
    } else {
        current_track.pause()?;

        msg.channel_id.say(&ctx.http, format!("Paused **{}**", current_track.metadata().title.clone().unwrap())).await?;
    }

    Ok(())
}

#[command]
#[description = "Plays a song, or enqueues it if a song is already playing"]
#[usage = "!play <song>"]
#[only_in(guilds)]
async fn play(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
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
    let handler = handler_lock.lock().await;
    let queue = handler.queue();
    let source: Restartable;
    let mut metadata: Metadata;

    if msg.attachments.len() > 0 {
        let mut handler = handler_lock.lock().await;
        let attachment = &msg.attachments[0];
        let file_name = attachment.filename.clone();
        let data = attachment.download().await.unwrap();
        let mut file = File::create(file_name.clone()).unwrap();
        file.write_all(&data)?;

        let source = Restartable::ffmpeg(file_name, true).await?;
        let (song, handle) = tracks::create_player(source.into());

        metadata = handle.metadata().to_owned();
        metadata.source_url.replace(msg.link());
        metadata.channel.replace(msg.author.name.clone());
        metadata.title.replace(attachment.filename.clone());

        handler.enqueue(song);
    } else {
        let mut handler = handler_lock.lock().await;
        let search = args.message().to_string();

        if search.len().eq(&0) {
            resume(ctx, msg, args.clone()).await.unwrap();
            return Ok(())
        }

        if search.starts_with("http") {
            source = Restartable::ytdl(search.clone(), true).await.unwrap();
        } else {
            source = Restartable::ytdl_search(&search, true).await.unwrap();
        }
        let song = handler.enqueue_source(source.into());
        drop(handler);
        metadata = song.metadata().to_owned();
    }    
    if queue.current().is_none() {
        ctx.online().await;

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
        msg.channel_id.send_message(ctx, |m| 
            m.content(format!("Enqueued **{}** by **{}**", metadata.title.clone().unwrap(), metadata.channel.clone().unwrap()))
            ).await.unwrap();
    }

    Ok(())
}


#[command]
#[description = "Gets the current queue"]
#[usage = "!queue"]
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

        if queue.len() == 0 || now_playing.is_none() {
            msg.channel_id.send_message(ctx, |m| m.content("Nothing in the queue")).await?;

            return Ok(())
        } 

        let now_playing = now_playing.unwrap();
        let metadata = now_playing.metadata();
        let extra_info = now_playing.get_info().await?;

        if queue.len() >= 2 {
            for (i, j) in queue.iter().zip(
                1..(queue.len() + 1)
            ) {
                if j == 1 { continue }
                description.push_str(
                    &format!("**{})** [{}]({})\n", j-1, i.metadata().title.clone().unwrap(), i.metadata().source_url.clone().unwrap()) as &str
                )
            }
        }
        msg.channel_id.send_message(ctx, |m| 
            m.embed(|e|
                e.field(
                    "• Now Playing",
                    format!("[{}]({}) [{}:{:#?}]", metadata.title.clone().unwrap(), metadata.source_url.clone().unwrap(), extra_info.play_time.as_secs(), metadata.duration.clone().unwrap()),
                    false
                )
                .field(
                    "• Up Next",
                    description.as_str(),
                    false
                )
                .color(Colour::GOLD)
            )).await?;


    } else {
        msg.channel_id
            .say(&ctx.http, "Not playing any music right now")
            .await?;
    }

    Ok(())
}

#[command]
#[description = "Skips the song currently playing and goes to the next in queue"]
#[usage = "!skip"]
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
        drop(&handler);
        if let None = queue.current() {
            msg.channel_id.say(ctx, "Not playing any music right now").await?;
            return Ok(())
        }
        let song = queue.current().unwrap();
        let _ = queue.skip();

        msg.channel_id
            .send_message(ctx, |m|
                m.embed(|e| 
                    e.description("Skipped {}.")
                    .color(Colour::GOLD)
                    .footer(|f|
                        f.text(format!("Invoked by {}", msg.author.name))
                        .icon_url(msg.author.face())
                    )
                )
            )
            .await?;
    } else {
        msg.channel_id
            .say(&ctx.http, "Not in a voice channel to play in")
            .await?;
    }

    Ok(())
}

#[command]
#[description = "Clears the queue and stops playing the current song. Also leaves the voice channel."]
#[usage = "!stop"]
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
        drop(&handler);
        queue.stop();

        msg.channel_id.say(&ctx.http, "Skipped song and cleared queue").await?;
    } else {
        msg.channel_id
            .say(&ctx.http, "Not in a voice channel to play in")
            .await?;
    }

    Ok(())
}

#[command]
#[description = "Resumes the current song after pausing"]
#[usage = "!resume"]
#[only_in(guilds)]
async fn resume(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;

        if handler.queue().current().is_none() {
            msg.channel_id.send_message(ctx, |m|
                m.content("Nothing playing currently")
            ).await?;
            return Ok(());
        }
    
        let current_track = handler.queue()
            .current()
            .unwrap();
    
        let play_status = current_track.get_info()
            .await
            .unwrap()
            .playing;

        drop(handler);

        if matches!(play_status, PlayMode::Pause) {
            current_track.play();
            msg.channel_id.send_message(ctx, |m|
                m.content(format!("Resumed **{}**", current_track.metadata().title.clone().unwrap()))
            ).await?;
        }

    } else {
        msg.channel_id
            .say(&ctx.http, "No music to resume")
            .await?;
    }

    Ok(())
}

#[command]
#[description = "Removes a song in the queue"]
#[usage = "!remove <number>"]
#[only_in(guilds)]
async fn remove(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if let Err(_s) = args.parse::<usize>() {
        msg.channel_id.send_message(ctx, |m|
            m.content("Please enter an index (e.g. `1`")
        ).await?;
        
        return Ok(());
    }

    let index = args.parse::<usize>().unwrap();

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice Client placed in at initialisation")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();

        if queue.len() < 1 {
            msg.channel_id.send_message(ctx, |m|
                m.content("Nothing in the queue")
            ).await?;
            
            return Ok(());
        }

        if let Some(song) = queue.dequeue(index) {
            msg.channel_id.send_message(ctx, |m|
                m.add_embed(|e|
                    e.title("Removed Song from Queue")
                        .description(format!("Removed **{:#?}** from the queue (", song.metadata().title))
                        .color(Colour::GOLD)
                        .footer(|f|
                            f.text(format!("Invoked by {}", msg.author.name))
                                .icon_url(msg.author.face())
                        )
                )).await?;
        } else {
            msg.channel_id.send_message(ctx, |m|
                m.content(format!("There is no song at index {}", index))
            ).await?;
            
            return Ok(());
        }
        
    } else {
        msg.channel_id.send_message(ctx, |m|
            m.content("Nothing playing currently")
        ).await?;
        
        return Ok(());
    }

    Ok(())
}

#[command("loop")]
#[description = "Enables or disables looping"]
#[usage = "!loop [mode] (\"current\" or \"disable\""]
#[only_in(guilds)]
async fn loop_command(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice Client placed in at initialisation")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();

        let loop_type: &str;
        let args = args.message();
    
        if args != "current" || args != "queue" {
            if queue.current().unwrap().get_info().await.unwrap().loops != LoopState::Infinite {
                loop_type = "current";
            } else {
                loop_type = "disable";
            }
        } else {
            loop_type = args;
        }

        if loop_type == "current" {
            queue.current().unwrap().enable_loop()?;
        } else {
            queue.current().unwrap().disable_loop()?;
        }

        msg.channel_id.send_message(ctx, |m|
            m.embed(|e|
                e.description(format!("Loop set to `{}`", loop_type))
                .color(Colour::GOLD)
            )
        ).await?;

    } else {
        msg.channel_id.send_message(ctx, |m|
            m.content("Nothing playing currently")
        ).await?;
    }

    Ok(())
}

#[command]
#[description = "Sets the volume of the currently playing track"]
#[usage = "!volume <number 1-100>"]
#[only_in(guilds)]
async fn volume(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    if let Err(_e) = args.message().parse::<f32>() {
        msg.channel_id.send_message(ctx, |m|
            m.content("Volume must be a number between `1-100`")
        ).await?;

        return Ok(());
    }

    let volume = args.message().parse::<f32>().unwrap();

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice Client placed in at initialisation")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let current = queue.current();

        if current.is_none() {
            msg.channel_id.send_message(ctx, |m|
                m.content("Nothing playing currently")
            ).await?;  
            
            return Ok(());
        }

        current.unwrap().set_volume(volume);

        msg.channel_id.send_message(ctx, |m|
            m.embed(|e|
                e.description(format!("Set the volume to `{}`", volume))
                    .color(Colour::GOLD)
            )
        ).await?;

    } else {
        msg.channel_id.send_message(ctx, |m|
            m.content("Nothing playing currently")
        ).await?;
    }


    Ok(())
}