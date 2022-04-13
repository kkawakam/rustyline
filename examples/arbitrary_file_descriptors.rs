use rustyline::{Behavior, Config, Editor, Result};
use std::fs::OpenOptions;
use std::io;

fn main() -> Result<()> {
    #![cfg(all(unix, not(target_arch = "wasm32")))]
    {
        use std::os::unix::io::IntoRawFd;

        let mut path = String::new();
        io::stdin().read_line(&mut path)?;
        let terminal = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.trim())?;
        let terminal_fd = terminal.into_raw_fd();
        let config = Config::builder()
            .behavior(Behavior::ArbitraryFileDescriptors {
                output: terminal_fd,
                input: terminal_fd,
            })
            .build();
        let mut rl = Editor::<()>::with_config(config);
        loop {
            let line = rl.readline("> ")?;
            println!("Line: {}", line);
        }
    }
}
