use rustyline::{
    Cmd, ConditionalEventHandler, Editor, Event, EventContext, EventHandler, KeyCode, KeyEvent,
    Modifiers, RepeatCount,
};

struct FilteringEventHandler;
impl ConditionalEventHandler for FilteringEventHandler {
    fn handle(&self, evt: &Event, _: RepeatCount, _: bool, _: &EventContext) -> Option<Cmd> {
        if let Some(KeyEvent(KeyCode::Char(c), m)) = evt.get(0) {
            if m.contains(Modifiers::CTRL) || m.contains(Modifiers::ALT) || c.is_ascii_digit() {
                None
            } else {
                Some(Cmd::Noop) // filter out invalid input
            }
        } else {
            None
        }
    }
}

fn main() {
    let mut rl = Editor::<()>::new();

    rl.bind_sequence(
        Event::Any,
        EventHandler::Conditional(Box::new(FilteringEventHandler)),
    );

    loop {
        let readline = rl.readline("> ");
        match readline {
            Ok(line) => {
                println!("Num: {}", line);
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
}
