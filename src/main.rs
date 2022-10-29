use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::ops::DerefMut;
use std::path::PathBuf;

use anyhow::anyhow;
use clap::Parser;
use hlua::{Lua, LuaError};
use noak::AccessFlags;
use noak::reader::{cpool, Class, Field, Attribute, AttributeContent};

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
        println!("Visiting file {}", file.name());
        if file.is_file() {
            if file.name().ends_with(".class") && class_name_evaluator.evaluate_if_class_needed(file.name())? {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                let mut class = Class::new(&*data)?;
                let is_enum = class.access_flags()?.contains(AccessFlags::ENUM);
                if is_enum {
                    process_enum(&mut class)?;
                }
            } else if file.name().ends_with(".jar") {
                println!("Parsing internal JAR: {}", file.name());
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                list_zip_contents(io::Cursor::new(data), class_name_evaluator)?;
                println!("---Leaving JAR {} ---", file.name());
            } else {
                println!("Ignored file: {}", file.name());
            }
        }
    }

    Ok(())
}

fn process_enum(class: &mut Class) -> anyhow::Result<()> {
    const AVRO_GENERATED: &str = "Lorg/apache/avro/specific/AvroGenerated;";
    const ENUM_MEMBER_FLAGS: AccessFlags = AccessFlags::PUBLIC
        .union(AccessFlags::STATIC)
        .union(AccessFlags::FINAL);
    let class_name = class.this_class_name()?.display();
    let internal_type_name = class.this_class_name()?.to_str().ok_or(anyhow!("Error decoding type name {}", class_name))?;
    let internal_type_name = format!("L{};", internal_type_name);
    println!("Class {} is ENUM", class_name);
    for field in class.fields()? {
        let fld: &Field = &field?;
        if fld.access_flags().contains(ENUM_MEMBER_FLAGS) {
            let pool = class.pool()?;
            let field_name: &cpool::Utf8 = pool.get(fld.name())?;
            let descriptor: &cpool::Utf8 = pool.get(fld.descriptor())?;
            if internal_type_name.eq(descriptor.content.to_str().unwrap_or("")) {
                println!("Enum member: {}", field_name.content.display());
            }
        }
    }

    for attr in class.attributes()? {
        let attribute: &Attribute = &attr?;
        let pool = class.pool()?;
        let attr_content = attribute.read_content(pool)?;
        match attr_content {
            AttributeContent::RuntimeInvisibleAnnotations(annotations)
            | AttributeContent::RuntimeVisibleAnnotations(annotations) => {
                for annotation in annotations.iter() {
                    let annotation_type: &cpool::Utf8  = pool.get(annotation?.type_())?;
                    if AVRO_GENERATED.eq(annotation_type.content.to_str().unwrap()) {
                        println!("Enum is AVRO-generated");
                    }
                }
            },
            _ => {}
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
