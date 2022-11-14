use rustyline::{Config, Editor, Result};

fn main() -> Result<()> {
    let config = Config::builder().auto_add_history(true).build();
    #[cfg(feature = "with-sqlite-history")]
    let history = if false {
        rustyline::sqlite_history::SQLiteHistory::with_config(config)? // memory
    } else {
        rustyline::sqlite_history::SQLiteHistory::open(config, "history.sqlite3")?
        // file
    };
    #[cfg(not(feature = "with-sqlite-history"))]
    let history = rustyline::history::MemHistory::with_config(config);
    let mut rl: Editor<(), _> = Editor::with_history(config, history)?;
    loop {
        let line = rl.readline("> ")?;
        println!("{line}");
    }
}
