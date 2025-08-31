use anyhow::Result;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, request::ChatMessageRequest},
};
use std::{collections::VecDeque, sync::Arc};
use teloxide::{
    dispatching::{HandlerExt, UpdateFilterExt},
    macros,
    prelude::*,
    types::{ChatAction, ParseMode},
    utils::command::BotCommands,
};
use tokio_stream::StreamExt;

mod history;
use history::History;

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

pub fn split_markdown_into_chunks(markdown: &str, max_chunk_size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut in_code_block = false;
    let mut lines: VecDeque<&str> = markdown.lines().collect();

    while let Some(line) = lines.pop_front() {
        // Check if this line starts or ends a code block
        let trimmed = line.trim();
        let is_code_fence = trimmed.starts_with("```");

        if is_code_fence {
            in_code_block = !in_code_block;
        }

        // Calculate what adding this line would do to the current chunk
        let line_with_newline = if lines.is_empty() {
            line.to_string()
        } else {
            format!("{}\n", line)
        };

        let potential_new_size = current_chunk.len() + line_with_newline.len();

        if potential_new_size <= max_chunk_size {
            // Safe to add the line
            current_chunk.push_str(&line_with_newline);
        } else {
            // Need to split
            if in_code_block && is_code_fence {
                // We're at a code fence boundary, safe to split here
                chunks.push(current_chunk);
                current_chunk = line_with_newline;
            } else if in_code_block {
                // We're inside a code block and need to split mid-block
                let remaining_capacity = max_chunk_size - current_chunk.len();

                if remaining_capacity >= 5 {
                    // Enough space for "```\n"
                    // Close the current code block
                    current_chunk.push_str("```\n");
                    chunks.push(current_chunk);

                    // Start new chunk with code block continuation
                    current_chunk = format!("```{}\n", &trimmed[3..]); // Preserve language if any

                    // Add the rest of the current line to the new chunk
                    let remaining_line = if line.len() > 3 { &line[3..] } else { "" };

                    if !remaining_line.is_empty() {
                        current_chunk.push_str(remaining_line);
                        if !lines.is_empty() {
                            current_chunk.push('\n');
                        }
                    }
                } else {
                    // Not enough space for closing fence, push current chunk and handle line
                    chunks.push(current_chunk);
                    current_chunk = line_with_newline;
                }
            } else {
                // Not in code block, just split at line boundary
                chunks.push(current_chunk);
                current_chunk = line_with_newline;
            }
        }
    }

    // Add the final chunk if it's not empty
    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

pub fn sanitize_text(s: &str) -> String {
    ["<p>", "</p>", "<br />", "<li>", "</li>", "<ol>", "</ol>"]
        .iter()
        .fold(markdown::to_html(s), |s, pattern| s.replace(pattern, ""))
}

async fn handle_msg(bot: Bot, msg: Message, state: Arc<State>) -> Result<()> {
    if let Some(text) = msg.text() {
        let usr = message_username(&msg);
        log::debug!("User <{usr}> send request: {text:.20}.");
        bot.send_chat_action(msg.chat.id, ChatAction::Typing)
            .await?;
        let chat_history = state.history.get(msg.chat.id).await;
        let mut stream = state
            .ollama
            .send_chat_messages_with_history_stream(
                chat_history.messages,
                ChatMessageRequest::new(
                    MODEL_NAME.to_string(),
                    vec![ChatMessage::user(text.to_string())],
                ),
            )
            .await?
            .map(|resp| resp.map(|resp| resp.message.content));
        if let Some(Ok(line)) = stream.next().await {
            let mut text = line;
            let message_id = bot.send_message(msg.chat.id, text.clone()).await?.id;
            while let Some(Ok(line)) = stream.next().await {
                text.push_str(&line);
                bot.edit_message_text(msg.chat.id, message_id, text.clone())
                    .await?;
            }
        }
    }
    Ok(())
}
