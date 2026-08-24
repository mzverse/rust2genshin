#![feature(rustc_private)]

mod asset;
mod parser;

use std::error::Error;
use std::ops::Deref;
use std::path::Path;
use std::string::String;
use clap::Parser;
use syn::Item;
use syn::spanned::Spanned;
use toml::{Table, Value};

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_hir;
extern crate rustc_middle;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    path: String,
}

fn main() {
    match _main() {
        Ok(_) => {}
        Err(e) => eprintln!("{}", e),
    }
}

fn _main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let path = Path::new(&args.path);

    let path_config = path.join("Cargo.toml");

    let mut main = "src/lib.rs".to_string();
    if path_config.exists() {
        let config: Table = toml::from_str(&std::fs::read_to_string(path_config)?)?;

        if let Some(lib) = config.get("lib") {
            let Value::Table(lib) = lib else {
                return Err("config.lib is not table".into());
            };

            if let Some(it) = lib.get("path") {
                if let Value::String(it) = it {
                    main = it.to_string();
                } else {
                    return Err("config.lib.path is not string".into());
                }
            }
        }
    }

    let file = syn::parse_file(main.as_str())?;
    for item in file.items {
        match item {
            Item::Const(_) => {}
            Item::Enum(_) => {}
            Item::ExternCrate(_) => {}
            Item::Fn(_) => {}
            Item::ForeignMod(_) => {}
            Item::Impl(_) => {}
            Item::Macro(_) => {}
            Item::Mod(_) => {}
            Item::Static(_) => {}
            Item::Struct(_) => {}
            Item::Trait(_) => {}
            Item::TraitAlias(_) => {}
            Item::Type(_) => {}
            Item::Union(_) => {}
            Item::Use(_) => {}
            it => println!("{:?}", it.span().start())
        }
    }
    
    Ok(())
}
