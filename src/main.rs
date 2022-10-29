use std::path::PathBuf;
use std::fs::{File, create_dir_all};
use std::io;
use clap::Parser;
use tempdir::TempDir;
use hlua::{Lua, LuaError};
use anyhow::anyhow;
use std::io::prelude::*;
use std::ops::DerefMut;

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
    debug: u8
}

trait ClassNameEvaluator {
    fn evaluate_if_class_needed(&mut self, _class_name: &str) -> anyhow::Result<bool> {
        return Ok(true);
    }
}

struct Dummy{}

impl ClassNameEvaluator for Dummy {}

struct LuaEvaluator<'a> {
    executor: Lua<'a>,
    luacode: &'a str
}

impl <'a>LuaEvaluator<'a> {
    pub fn new(code: &'a str) -> Self {
        let mut res = LuaEvaluator {
            executor: Lua::new(),
            luacode: code
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

fn pause() {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    // We want the cursor to stay at the end of the line, so we print without a newline and flush manually.
    write!(stdout, "Press any key to continue...").unwrap();
    stdout.flush().unwrap();

    // Read a single byte and discard
    let _ = stdin.read(&mut [0u8]).unwrap();
}

fn list_zip_contents(reader: impl Read + Seek, tmp_dir: &TempDir, class_name_evaluator: &mut dyn ClassNameEvaluator) -> anyhow::Result<()> {
    let mut zip = zip::ZipArchive::new(reader)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.is_file() {
            if class_name_evaluator.evaluate_if_class_needed(file.name())? {
                let path_to_extract = tmp_dir.path().join(file.mangled_name());
                println!("Filename: {} is extracted to {}", file.name(), path_to_extract.display());
                match path_to_extract.parent() {
                    Some(path) => {
                        println!("Will try to create path {}", path.display());
                        create_dir_all(path)?;
                    }
                    None => {}
                }
                println!("About to create tmp file");
                let mut target = File::create(path_to_extract)?;
                let mut data = Vec::new();
                println!("About to read from zip");
                file.read_to_end(&mut data)?;
                println!("About to write to tmp");
                target.write_all(&data)?;
            }
        }
    }


    pause();
    Ok(())
}

fn main() -> anyhow::Result<()> {
  let cli = Cli::parse();

  println!("JAR file to scan: {}", cli.jarfile.display());

  let tmp_dir = TempDir::new("jar-scanner")?;
  
  {

    let jarfile = cli.jarfile;
    let jarfile = File::open(jarfile)?;


    let mut evaluator: Box<dyn ClassNameEvaluator> =
          match cli.luaclassfilter.as_ref() {
              Some(code) => Box::new(LuaEvaluator::new(code)),
              None => Box::new(Dummy{})
          };
    list_zip_contents(jarfile, &tmp_dir, evaluator.deref_mut())?;
  }

  tmp_dir.close()?;
  Ok(())
}
