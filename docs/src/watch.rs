use crate::typst::{SRC, build, run};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::process::exit;

pub mod typst;

fn main() {
    build();
    let mut watcher = notify::recommended_watcher(|res: notify::Result<Event>| {
        match res {
            Ok(event) => match event.kind {
                | EventKind::Create(_)
                | EventKind::Remove(_)
                if event.paths.iter().any(|p| !p.is_dir() && !if let Some(s) = p.to_str() && s.ends_with("~") { true } else { false } )
                => build(),
                _ => {}
            },
            Err(e) => println!("watch error: {:?}", e),
        }
    }).unwrap();
    watcher.watch(&SRC, RecursiveMode::Recursive).unwrap();
    exit(run("watch"));
}
