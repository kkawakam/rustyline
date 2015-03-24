extern crate rustyline;

fn main() {
    let readline = rustyline::readline(">> ");
    match readline {
        Some(line) => println!("Line: {:?}",line),
        None       => println!("No input"),
    }
}
