extern crate rustyline;

fn main() {
    let mut rl = rustyline::Editor::new();
    let readline = rl.readline(">> ");
    match readline {
        Ok(line) => println!("Line: {}",line),
        Err(_)   => println!("No input"),
    }
}
