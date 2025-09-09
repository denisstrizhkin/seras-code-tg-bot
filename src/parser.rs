use anyhow::Result;
use std::fmt;
use tokio::io::{AsyncBufRead, Lines};

use crate::util::truncate_str;

const CODE_GUARD: &str = "```";
const MSG_CHUNK_LEN: usize = 500;
const MSG_MAX_LEN: usize = 4000;

#[derive(Default)]
pub struct MessageParserState {
    pub buffer: String,
    pub text: String,
    pub is_complete: bool,
    lang: String,
    is_in_code_block: bool,
    chunk_goal_n: usize,
}

impl fmt::Debug for MessageParserState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageParserState")
            .field("buffer", &truncate_str(&self.buffer, 20))
            .field("text", &truncate_str(&self.text, 20))
            .field("is_complete", &self.is_complete)
            .field("lang", &self.lang)
            .field("is_in_code_block", &self.is_in_code_block)
            .field("chunk_goal_n", &self.chunk_goal_n)
            .finish()
    }
}

impl MessageParserState {
    #[inline(always)]
    fn check_overflow(&self, line: &str) -> bool {
        self.buffer.chars().count() + line.chars().count() > MSG_MAX_LEN
    }

    #[inline(always)]
    fn get_n_chunks(&self) -> usize {
        self.buffer.chars().count() / MSG_CHUNK_LEN
    }

    #[inline(always)]
    fn insert_line(&mut self, line: &str) {
        self.buffer.push_str(line);
        self.buffer.push('\n');
    }

    #[inline(always)]
    fn finalize(&mut self) {
        if self.is_in_code_block {
            self.insert_line(CODE_GUARD);
        }
        self.text = std::mem::take(&mut self.buffer);
        self.is_complete = true;
    }

    fn handle(&mut self, line: &str) {
        let is_overflow = self.check_overflow(line);
        if is_overflow {
            self.finalize();
        } else {
            self.is_complete = false;
        }
        if let Some(lang) = line.strip_prefix(CODE_GUARD) {
            self.is_in_code_block = !self.is_in_code_block;
            if self.is_in_code_block {
                self.lang = lang.to_string()
            }
        } else if is_overflow && self.is_in_code_block {
            self.insert_line(&format!("{CODE_GUARD}{}", self.lang));
        }
        self.insert_line(line);
    }
}

pub struct MessageParser<R> {
    lines: Lines<R>,
    state: MessageParserState,
}

impl<R: AsyncBufRead + Unpin> MessageParser<R> {
    pub fn new(lines: Lines<R>) -> Self {
        Self {
            lines,
            state: MessageParserState {
                chunk_goal_n: 1,
                ..Default::default()
            },
        }
    }

    pub async fn next_state(&mut self) -> Result<Option<&MessageParserState>> {
        while let Some(line) = self.lines.next_line().await? {
            self.state.handle(&line);
            if self.state.get_n_chunks() >= self.state.chunk_goal_n {
                self.state.chunk_goal_n += 1;
                return Ok(Some(&self.state));
            } else if self.state.is_complete {
                self.state.chunk_goal_n = 1;
                return Ok(Some(&self.state));
            }
        }
        Ok(if self.state.is_complete {
            None
        } else {
            self.state.finalize();
            Some(&self.state)
        })
    }
}
