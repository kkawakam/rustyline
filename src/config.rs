//! Customize line editor
use std::default::Default;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// When listing completion alternatives, only display
    /// one screen of possibilities at a time.
    max_history_size: usize,
    history_duplicates: HistoryDuplicates,
    history_ignore_space: bool,
    completion_type: CompletionType,
    /// When listing completion alternatives, only display
    /// one screen of possibilities at a time.
    completion_prompt_limit: usize,
    /// Duration (milliseconds) Rustyline will wait for a character when reading an ambiguous key sequence.
    keyseq_timeout: i32,
}

impl Config {
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Tell the maximum length for the history.
    pub fn max_history_size(&self) -> usize {
        self.max_history_size
    }

    /// Tell if lines which match the previous history entry are saved or not in the history list.
    /// By default, they are ignored.
    pub fn history_duplicates(&self) -> HistoryDuplicates {
        self.history_duplicates
    }

    /// Tell if lines which begin with a space character are saved or not in the history list.
    /// By default, they are saved.
    pub fn history_ignore_space(&self) -> bool {
        self.history_ignore_space
    }

    pub fn completion_type(&self) -> CompletionType {
        self.completion_type
    }

    pub fn completion_prompt_limit(&self) -> usize {
        self.completion_prompt_limit
    }

    pub fn keyseq_timeout(&self) -> i32 {
        self.keyseq_timeout
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            max_history_size: 100,
            history_duplicates: HistoryDuplicates::IgnoreConsecutive,
            history_ignore_space: false,
            completion_type: CompletionType::Circular, // TODO Validate
            completion_prompt_limit: 100,
            keyseq_timeout: 500,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryDuplicates {
    AlwaysAdd,
    IgnoreConsecutive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionType {
    /// Complete the next full match (like in Vim by default)
    Circular,
    /// Complete till longest match.
    /// When more than one match, list all matches
    /// (like in Bash/Readline).
    List,
}

#[derive(Debug)]
pub struct Builder {
    p: Config,
}

impl Builder {
    pub fn new() -> Builder {
        Builder { p: Config::default() }
    }

    /// Set the maximum length for the history.
    pub fn max_history_size(mut self, max_size: usize) -> Builder {
        self.p.max_history_size = max_size;
        self
    }

    /// Tell if lines which match the previous history entry are saved or not in the history list.
    /// By default, they are ignored.
    pub fn history_ignore_dups(mut self, yes: bool) -> Builder {
        self.p.history_duplicates = if yes {
            HistoryDuplicates::IgnoreConsecutive
        } else {
            HistoryDuplicates::AlwaysAdd
        };
        self
    }

    /// Tell if lines which begin with a space character are saved or not in the history list.
    /// By default, they are saved.
    pub fn history_ignore_space(mut self, yes: bool) -> Builder {
        self.p.history_ignore_space = yes;
        self
    }

    /// Set `completion_type`.
    pub fn completion_type(mut self, completion_type: CompletionType) -> Builder {
        self.p.completion_type = completion_type;
        self
    }

    pub fn completion_prompt_limit(mut self, completion_prompt_limit: usize) -> Builder {
        self.p.completion_prompt_limit = completion_prompt_limit;
        self
    }

    /// Set `keyseq_timeout` in milliseconds.
    pub fn keyseq_timeout(mut self, keyseq_timeout_ms: i32) -> Builder {
        self.p.keyseq_timeout = keyseq_timeout_ms;
        self
    }

    pub fn build(self) -> Config {
        self.p
    }
}
