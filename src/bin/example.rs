extern crate rustyline;

fn main() {
    let readline = rustyline::readline(">> ");
    match readline {
        Ok(line) => println!("Line: {}",line),
        Err(_)   => println!("No input"),
    }
}
