use anyhow::Result;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, request::ChatMessageRequest},
};
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::{
    dispatching::{HandlerExt, UpdateFilterExt},
    macros,
    prelude::*,
    types::ChatAction,
    utils::command::BotCommands,
};
use tokio::sync::RwLock;

const MODEL_NAME: &str = "qwen2.5-coder:32b";

struct State {
    ollama: Ollama,
    history: RwLock<HashMap<ChatId, Vec<ChatMessage>>>,
}

impl State {
    fn new() -> Self {
        Self {
            ollama: Ollama::default(),
            history: RwLock::new(HashMap::new()),
        }
    }
}

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
    .dependencies(dptree::deps![Arc::new(State::new())])
    .enable_ctrlc_handler()
    .build()
    .dispatch()
    .await;
    Ok(())
}

fn message_username(msg: &Message) -> String {
    msg.from
        .as_ref()
        .and_then(|x| x.username.clone())
        .unwrap_or_default()
}

async fn handle_help(bot: Bot, msg: Message) -> Result<()> {
    bot.send_message(msg.chat.id, "<Help message here>").await?;
    Ok(())
}

async fn handle_clear(bot: Bot, msg: Message, state: Arc<State>) -> Result<()> {
    state
        .history
        .write()
        .await
        .entry(msg.chat.id)
        .and_modify(|x| x.clear());
    let usr = message_username(&msg);
    log::debug!("Clear history for user <{usr}>.");
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
        format!("Unknown command: {cmd}. Use /help to see available commands."),
    )
    .await?;
    Ok(())
}

async fn handle_msg(bot: Bot, msg: Message, state: Arc<State>) -> Result<()> {
    if let Some(text) = msg.text() {
        let usr = message_username(&msg);
        log::debug!("User <{usr}> send request: {text:.20}.");
        bot.send_chat_action(msg.chat.id, ChatAction::Typing)
            .await?;
        let response = {
            let mut history_lock = state.history.write().await;
            let history = history_lock.entry(msg.chat.id).or_default();
            state
                .ollama
                .send_chat_messages_with_history(
                    history,
                    ChatMessageRequest::new(
                        MODEL_NAME.to_string(),
                        vec![ChatMessage::user(text.to_string())],
                    ),
                )
                .await?
        };
        log::debug!(
            "Model responded to user <{usr}>: {:.20}.",
            response.message.content
        );
        bot.send_message(msg.chat.id, response.message.content)
            .await?;
    }
    Ok(())
}
