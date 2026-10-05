#![feature(rustc_private)]

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use bincode::de::read::Reader;
use rust2genshin::asset::GameMode;
use rust2genshin::compile::link::{Linker, Target};

fn main() {
    let mut inputs = Vec::new();
    let mut output = None;
    let mut gamemode = GameMode::Beyond;
    {
        let mut it = std::env::args();
        it.next(); // self
        while let Some(arg) = it.next() {
            if arg.starts_with("-") {
                let arg = &arg.as_str()[1..];
                if let x = arg.chars().next().unwrap() && x.is_uppercase() {
                    let mut arg = &arg[1..];
                    let arg_owned;
                    if arg.is_empty() {
                        arg_owned = it.next().unwrap();
                        arg = &arg_owned;
                    }
                    match x {
                        'W' => (),
                        'L' => (),
                        _ => panic!("{x} = {arg}"),
                    }
                } else {
                    match arg {
                        "o" => {
                            assert!(output.is_none());
                            output = it.next().unwrap().into();
                        },
                        "shared" |
                        concat!("no", "default", "libs") => (),
                        "gamemode" => {
                            gamemode = match it.next().unwrap().as_str() {
                                "beyond" => GameMode::Beyond,
                                "classic" => GameMode::Classic,
                                _ => panic!(),
                            };
                        }
                        _ => panic!("{arg}"),
                    }
                }
            } else {
                inputs.push(arg);
            }
        }
    }

    let mut target = Target::default();
    fn handle_input(name: impl AsRef<Path>, reader: impl Reader) -> Target {
        match name.as_ref().extension().unwrap().to_str().unwrap() {
            "ogia" => bincode::serde::decode_from_reader(reader, bincode::config::standard()).unwrap(),
            _ => Target::default(),
        }
    }
    for x in inputs {
        let path = PathBuf::from(x);
        match path.extension().unwrap().to_str().unwrap() {
            "rlib" => {
                let mut archive = ar::Archive::new(BufReader::new(File::open(path).unwrap()));
                while let Some(e) = archive.next_entry() {
                    let mut entry = e.unwrap();
                    let mut content = Vec::new();
                    entry.read_to_end(&mut content).unwrap();
                    target += handle_input(String::from_utf8(Vec::from(entry.header().identifier())).unwrap(), BufReader::new(&content[..]));
                }
            },
            _ => target += handle_input(&path, BufReader::new(File::open(&path).unwrap())),
        }
    }
    Linker::new(gamemode, target, PathBuf::from(output.unwrap())).link();
}
