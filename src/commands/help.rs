use std::collections::HashSet;

use serenity::framework::standard::macros::{help};
use serenity::framework::standard::{
    help_commands,
    help_commands::CustomisedHelpData,
    Args,
    CommandGroup,
    CommandResult,
    HelpOptions,
};
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::utils::Colour;

use crate::utils::utilities::rand_color;


#[help]
#[individual_command_tip = "To view help for all categories and commands, type `!help`\nTo view help for a category or command, type `!help [command/category]`"]
#[command_not_found_text = "There is no command called `\'{}\'`. View `/help` or `!help` to view all commands and categories."]
#[max_levenshtein_distance(3)]
async fn help(    
    ctx: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,        
) -> CommandResult {
    let help_data = help_commands::create_customised_help_data(ctx, msg, &args, groups, &owners, help_options).await;

    let _ = msg.channel_id.send_message(ctx, |m| {
        m.embed(|e| {
            match help_data {
                // When similar commands where found to user's query
                CustomisedHelpData::SuggestedCommands { help_description: _, suggestions } => 
                e.description(format!("There are no commands that match your search. Did you mean {}?", suggestions.join(", ")))
                .color(Colour::RED)
                .footer(|f| 
                    f.text(format!("Requested by {}", msg.author.name))
                    .icon_url(msg.author.face())
                    ),

                // When running !help with no arguments (displaying all commands)
                CustomisedHelpData::GroupedCommands { help_description, groups } => {
                    e.title("**Bot Commands**")
                    .description(help_description)
                    .color(rand_color())
                    .footer(|f| 
                        f.text(format!("Requested by {}", msg.author.name))
                        .icon_url(msg.author.face())
                        );

                    for i in 0..groups.len() {
                        e.field(groups[i].name, groups[i].command_names.join("\n"), true);
                    }

                    e
                },

                // When getting help for a specific command
                CustomisedHelpData::SingleCommand { command } => 
                    e.title(format!("{:#?}", command.usage.unwrap()))
                    .description("To view help for all categories and commands, type `!help`\nTo view help for a category or command, type `!help [command/category]`")
                    .color(rand_color())
                    .field("Description", format!("{:#?}", command.description.unwrap().replace("\n", "").replace("\"", "")), false)
                    .field("Category", command.group_name, false),

                // When no command matches or comes close to user's query
                CustomisedHelpData::NoCommandFound { help_error_message } => e.description(help_error_message)
                    .color(Colour::RED)
                    .footer(|f| 
                        f.text(format!("Requested by {}", msg.author.name))
                        .icon_url(msg.author.face())
                    ),

                // No idea how this can be called
                _ =>
                    e.description("What are you trying to do?")
                    .color(Colour::RED)
                    .footer(|f| 
                        f.text(format!("Requested by {}", msg.author.name))
                        .icon_url(msg.author.face())
                    )
            }

        })
    }).await;
    Ok(())
}