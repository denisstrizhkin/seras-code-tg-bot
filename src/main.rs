use anyhow::{Result, anyhow};
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, request::ChatMessageRequest},
};
use std::sync::Arc;
use teloxide::{
    dispatching::{HandlerExt, UpdateFilterExt},
    macros,
    payloads::SendMessageSetters,
    prelude::*,
    types::{ChatAction, MessageId, ParseMode},
    utils::command::BotCommands,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_stream::StreamExt;
use tokio_util::{bytes, io::StreamReader};

mod history;
mod parser;
mod util;

use history::History;
use parser::MessageParser;
use util::truncate_str;

const MODEL_NAME: &str = "qwen2.5-coder:32b";

#[derive(Default)]
struct State {
    ollama: Ollama,
    history: History,
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
    .dependencies(dptree::deps![Arc::new(State::default())])
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
    state.history.clear(msg.chat.id).await;
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
    let cmd = truncate_str(text, 50);
    bot.send_message(
        msg.chat.id,
        format!("Unknown command: {cmd}. Use /help to see available commands."),
    )
    .await?;
    Ok(())
}

pub fn sanitize_text(s: &str) -> String {
    [
        "<p>", "</p>", "<br />", "<li>", "</li>", "<ol>", "</ol>", "<h1>", "</h1>", "<h2>",
        "</h2>", "<h3>", "</h3>", "<h4>", "</h4>", "<h5>", "</h5>", "<ul>", "</ul>",
    ]
    .iter()
    .fold(markdown::to_html(s), |s, pattern| s.replace(pattern, ""))
}

async fn handle_msg(bot: Bot, msg: Message, state: Arc<State>) -> Result<()> {
    if let Some(text) = msg.text() {
        let usr = message_username(&msg);
        log::debug!("User <{usr}> send request: {}.", truncate_str(text, 20));
        let chat_history = state.history.get(msg.chat.id).await;
        let stream = state
            .ollama
            .send_chat_messages_with_history_stream(
                chat_history.messages,
                ChatMessageRequest::new(
                    MODEL_NAME.to_string(),
                    vec![ChatMessage::user(text.to_string())],
                ),
            )
            .await?
            .map(|resp| {
                resp.map(|resp| bytes::Bytes::from(resp.message.content.as_bytes().to_owned()))
                    .map_err(|_| std::io::Error::other(anyhow!("")))
            });
        let mut parser = MessageParser::new(BufReader::new(StreamReader::new(stream)).lines());
        let mut msg_id = None;
        while let Some(state) = {
            bot.send_chat_action(msg.chat.id, ChatAction::Typing)
                .await?;
            parser.next_state().await?
        } {
            log::debug!("User <{usr}> response: {state:?}");
            if state.is_complete {
                handle_complete_state(&bot, msg.chat.id, &mut msg_id, &state.text).await?;
            } else {
                handle_incomplete_state(&bot, msg.chat.id, &mut msg_id, &state.buffer).await?;
            }
        }
    }
    Ok(())
}

async fn handle_complete_state(
    bot: &Bot,
    chat_id: ChatId,
    msg_id: &mut Option<MessageId>,
    text: &str,
) -> Result<()> {
    if let Some(id) = msg_id.take() {
        bot.edit_message_text(chat_id, id, sanitize_text(text))
            .parse_mode(ParseMode::Html)
            .await?;
    } else {
        bot.send_message(chat_id, sanitize_text(text))
            .parse_mode(ParseMode::Html)
            .await?;
    }
    Ok(())
}

async fn handle_incomplete_state(
    bot: &Bot,
    chat_id: ChatId,
    msg_id: &mut Option<MessageId>,
    text: &str,
) -> Result<()> {
    if let Some(msg_id) = &msg_id {
        bot.edit_message_text(chat_id, *msg_id, text).await?;
    } else {
        *msg_id = Some(bot.send_message(chat_id, text).await?.id);
    }
    Ok(())
}
