use anyhow::Result;
use std::mem;
use teloxide::{
    dispatching::{HandlerExt, MessageFilterExt, UpdateFilterExt},
    macros,
    prelude::*,
    utils::command::BotCommands,
};

/// These commands are supported:
#[derive(macros::BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    /// Help message ^-^
    #[command(aliases = ["h", "?"])]
    Help,
    /// Clear context
    #[command(alias = "c")]
    Clear,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("Starting the bot...");
    let bot = Bot::from_env();
    bot.set_my_commands(Command::bot_commands()).await?;
    log::info!("Finished setting up the bot...");
    Dispatcher::builder(
        bot,
        Update::filter_message()
            .branch(
                dptree::entry()
                    .filter_command::<Command>()
                    .branch(dptree::case![Command::Help].endpoint(handle_help))
                    .branch(dptree::case![Command::Clear].endpoint(handle_clear)),
            )
            .branch(
                dptree::filter(|msg: Message| msg.text().is_some_and(|x| x.starts_with("/")))
                    .endpoint(handle_uknown_command),
            )
            .endpoint(handle_msg),
    )
    .enable_ctrlc_handler()
    .build()
    .dispatch()
    .await;
    Ok(())
}

async fn handle_help(bot: Bot, msg: Message, cmd: Command) -> Result<()> {
    bot.send_message(msg.chat.id, "<Help message here>").await?;
    Ok(())
}

async fn handle_clear(bot: Bot, msg: Message, cmd: Command) -> Result<()> {
    bot.send_message(msg.chat.id, "Context cleared.").await?;
    Ok(())
}

async fn handle_uknown_command(bot: Bot, msg: Message) -> Result<()> {
    let text = msg
        .text()
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default();
    let cmd = if text.chars().count() > 50 {
        format!("{}...", &text[..47])
    } else {
        text.to_string()
    };
    bot.send_message(
        msg.chat.id,
        format!(
            "Unknown command: {}. Use /help to see available commands.",
            cmd
        ),
    )
    .await?;
    Ok(())
}

async fn handle_msg(bot: Bot, msg: Message) -> Result<()> {
    Ok(())
}
