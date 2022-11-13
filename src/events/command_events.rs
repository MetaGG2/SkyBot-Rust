use std::fs;
use std::io::Write;
use std::time::Duration;

use serenity::collector::ReactionCollectorBuilder;
use serenity::framework::standard::{CommandResult};
use serenity::framework::standard::macros::{hook};
use serenity::futures::StreamExt;
use serenity::model::channel::Message;
use serenity::prelude::*;

use chrono::Local;


#[hook]
pub async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open("cmds")
        .unwrap();

    if let Err(why) = write!(
        file,
        "{} command: Used by {:#?} on {} with arguments \"{}\" in channel {:#?}\n", 
            command_name, 
            (msg.author.name.clone() + msg.author.discriminator.to_string().as_str()), 
            Local::now().format("%a %v %l:%M:%S %p"),
            msg.content.trim_end(), 
            msg.channel_id.name(ctx).await.unwrap()
    ) {
        println!("An Internal Error has occured\n{:#?}\n{:#?}", why, why.kind());
    }
    true
}

#[hook]
pub async fn after(ctx: &Context, msg: &Message, _command_name: &str, command_result: CommandResult) {
    if let Err(why) = command_result {
            let print_string = format!("{:#?}\n{:#?}", why, why.source());
            println!("{}", print_string);

            let message = msg.channel_id.send_message(ctx, |m| 
                m.content("An Internal Error has occured.")).await.unwrap();

            message.react(ctx, '❔').await;

            let collector = ReactionCollectorBuilder::new(ctx)
                .message_id(message.clone().id)
                .timeout(Duration::from_secs(60))
                .collect_limit(1)
                .build();
            
            let message = &message;
            let print_string = &print_string as &str;

            let _: Vec<_> = collector.then(|_msg| async move {
                let mut message = message.clone();
                message.edit(ctx, |m| 
                    m.content(format!("```{:#?}```",  print_string))
                ).await;

                _msg
            }).collect().await;
        }
}