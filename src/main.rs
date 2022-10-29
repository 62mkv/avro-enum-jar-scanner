use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::ops::DerefMut;
use std::path::PathBuf;

use anyhow::anyhow;
use clap::Parser;
use hlua::{Lua, LuaError};
use noak::AccessFlags;
use noak::reader::Class;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// jar-file to process
    #[arg(short, long, value_name = "FILE")]
    jarfile: PathBuf,

    #[arg(short, long, value_name = "LUACLASSFILTER")]
    luaclassfilter: Option<String>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}

trait ClassNameEvaluator {
    fn evaluate_if_class_needed(&mut self, _class_name: &str) -> anyhow::Result<bool> {
        return Ok(true);
    }
}

struct Dummy {}

impl ClassNameEvaluator for Dummy {}

struct LuaEvaluator<'a> {
    executor: Lua<'a>,
    luacode: &'a str,
}

impl<'a> LuaEvaluator<'a> {
    pub fn new(code: &'a str) -> Self {
        let mut res = LuaEvaluator {
            executor: Lua::new(),
            luacode: code,
        };
        res.executor.openlibs();
        res
    }
}

impl ClassNameEvaluator for LuaEvaluator<'_> {
    fn evaluate_if_class_needed(&mut self, class_name: &str) -> anyhow::Result<bool> {
        self.executor.set("classname", class_name);
        match self.executor.execute(self.luacode) {
            Ok(res) => anyhow::Ok(res),
            Err(LuaError::ExecutionError(error)) => Err(anyhow!("Error executing Lua code {}", error)),
            Err(LuaError::SyntaxError(error)) => Err(anyhow!("Error executing Lua code {}", error)),
            Err(LuaError::ReadError(error)) => Err(anyhow!("Error executing Lua code {}", error)),
            Err(LuaError::WrongType) => Err(anyhow!("Error executing Lua code: wrong type")),
        }
    }
}

fn list_zip_contents(reader: impl Read + Seek, class_name_evaluator: &mut dyn ClassNameEvaluator) -> anyhow::Result<()> {
    let mut zip = zip::ZipArchive::new(reader)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.is_file() {
            if file.name().ends_with(".class") && class_name_evaluator.evaluate_if_class_needed(file.name())? {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                let mut class = Class::new(&*data)?;
                let class_name = class.this_class_name()?.display();
                let is_enum = class.access_flags()?.contains(AccessFlags::ENUM);
                if is_enum {
                    println!("Class {} is ENUM", class_name);
                }
            } else if file.name().ends_with(".jar") {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                list_zip_contents(io::Cursor::new(data), class_name_evaluator)?;
            }
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    println!("JAR file to scan: {}", cli.jarfile.display());

    let jarfile = cli.jarfile;
    let jarfile = File::open(jarfile)?;


    let mut evaluator: Box<dyn ClassNameEvaluator> =
        match cli.luaclassfilter.as_ref() {
            Some(code) => Box::new(LuaEvaluator::new(code)),
            None => Box::new(Dummy {})
        };

    list_zip_contents(jarfile, evaluator.deref_mut())?;
    Ok(())
}
