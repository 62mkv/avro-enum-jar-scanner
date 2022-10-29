use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::PathBuf;

use anyhow::anyhow;
use clap::Parser;
use regex::Regex;
use noak::AccessFlags;
use noak::reader::{cpool, Class, Field, Attribute, AttributeContent};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// jar-file to process
    #[arg(short, long, value_name = "JAR_FILE")]
    jarfile: PathBuf,

    #[arg(short, long, value_name = "REGEX")]
    class_name_regex: Option<Regex>
}

struct RegexEvaluator {
    class_name_regex: Option<Regex>
}

impl RegexEvaluator {
    pub fn new(class_name_regex: Option<Regex>) -> Self {
        RegexEvaluator {
            class_name_regex
        }
    }

    fn evaluate_if_class_needed(&self, class_name: &str) -> anyhow::Result<bool> {
        Ok(self.class_name_regex.as_ref().map(|r| r.is_match(class_name)).unwrap_or(false))
    }
}

fn list_zip_contents(reader: impl Read + Seek, jarname: &str, class_name_evaluator: &RegexEvaluator) -> anyhow::Result<()> {
    let mut zip = zip::ZipArchive::new(reader)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.is_file() {
            let class_file_name = &(file.name().to_owned());
            let class_file_name = class_file_name.trim_start_matches("BOOT-INF/classes/");
            if class_file_name.ends_with(".class") && class_name_evaluator.evaluate_if_class_needed(class_file_name)? {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                let mut class = Class::new(&*data)?;
                let is_enum = class.access_flags()?.contains(AccessFlags::ENUM);
                if is_enum {
                    println!("Processing enum from {}", jarname);
                    process_enum(&mut class)?;
                }
            } else if class_file_name.ends_with(".jar") {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                list_zip_contents(io::Cursor::new(data), file.name(), class_name_evaluator)?;
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


    let evaluator = RegexEvaluator::new(cli.class_name_regex);

    list_zip_contents(jarfile, "root", &evaluator)?;
    Ok(())
}
