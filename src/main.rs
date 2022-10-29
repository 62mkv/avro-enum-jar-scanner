use std::fmt::{Display, Formatter};
use std::fmt;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::PathBuf;

use anyhow::anyhow;
use clap::builder::Str;
use clap::Parser;
use noak::AccessFlags;
use noak::reader::{Attribute, AttributeContent, Class, cpool, Field};
use regex::Regex;

use evaluator::RegexEvaluator;

mod evaluator;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// jar-file to process
    #[arg(short, long, value_name = "JAR_FILE")]
    jarfile: PathBuf,

    #[arg(short, long, value_name = "REGEX")]
    class_name_regex: Option<Regex>
}

enum ClassSource {
    Root,
    NestedJar(String)
}

impl Display for ClassSource {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ClassSource::Root => { write!(f, "root") },
            ClassSource::NestedJar(ref jar_name) => { write!(f, "{}", jar_name)}
        }
    }
}

struct EnumVisited {
    class_name: String,
    members: Vec<String>,
    avro_generated: bool,
    source: ClassSource
}

struct EnumVisitor {
    enums: Vec<EnumVisited>
}

impl<'a> EnumVisitor {
    pub fn new() -> Self {
        EnumVisitor {
            enums: Vec::new()
        }
    }

    pub fn visit_enum(self: &mut EnumVisitor, class: &'a mut Class<'a>, source: &'a ClassSource) -> anyhow::Result<()> {
        const AVRO_GENERATED: &str = "Lorg/apache/avro/specific/AvroGenerated;";
        const ENUM_MEMBER_FLAGS: AccessFlags = AccessFlags::PUBLIC
            .union(AccessFlags::STATIC)
            .union(AccessFlags::FINAL);
        println!("Processing enum from {}", source);
        let class_name = class.this_class_name()?;
        let internal_type_name = class.this_class_name()?.to_str().ok_or(anyhow!("Error decoding type name {}", class_name.display()))?;
        let internal_type_name = format!("L{};", internal_type_name);
        println!("Class {} is ENUM", class_name.display());
        let mut enum_members: Vec<String> = Vec::new();
        for field in class.fields()? {
            let fld: &Field = &field?;
            if fld.access_flags().contains(ENUM_MEMBER_FLAGS) {
                let pool = class.pool()?;
                let field_name: &cpool::Utf8 = pool.get(fld.name())?;
                let descriptor: &cpool::Utf8 = pool.get(fld.descriptor())?;
                if internal_type_name.eq(descriptor.content.to_str().unwrap_or("")) {
                    enum_members.push(field_name.content.to_str().unwrap().to_string());
                    println!("Enum member: {}", field_name.content.display());
                }
            }
        }
        let mut is_avro_generated = false;
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
                            is_avro_generated = true;
                            println!("Enum is AVRO-generated");
                        }
                    }
                },
                _ => {}
            }
        }

        let source = match source {
            ClassSource::Root => { ClassSource::Root },
            ClassSource::NestedJar(filename) => { ClassSource::NestedJar(String::from(filename))}
        };

        let enum_visited = EnumVisited {
            class_name: String::from(class_name.to_str().unwrap()),
            members: enum_members,
            avro_generated: is_avro_generated,
            source
        };

        self.enums.push(enum_visited);
        Ok(())
    }
}

fn list_zip_contents(reader: impl Read + Seek, source: &ClassSource, class_name_evaluator: &RegexEvaluator, enum_visitor: &mut EnumVisitor) -> anyhow::Result<()> {
    let mut zip = zip::ZipArchive::new(reader)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.is_file() {
            let class_file_name = &(file.name().to_owned());
            let class_file_name = if matches!(source, ClassSource::Root) {
                class_file_name.trim_start_matches("BOOT-INF/classes/")
            } else {
                class_file_name
            };
            if class_file_name.ends_with(".class") && class_name_evaluator.evaluate_if_class_needed(class_file_name)? {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                let mut class = Class::new(&*data)?;
                let is_enum = class.access_flags()?.contains(AccessFlags::ENUM);
                if is_enum {
                    enum_visitor.visit_enum(&mut class, source)?;
                }
            } else if class_file_name.ends_with(".jar") {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                list_zip_contents(io::Cursor::new(data), &ClassSource::NestedJar(String::from(file.name())), class_name_evaluator, enum_visitor)?;
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


    let evaluator = RegexEvaluator::new(cli.class_name_regex);

    let mut visitor = EnumVisitor::new();
    list_zip_contents(jarfile, &ClassSource::Root, &evaluator, &mut visitor)?;

    println!("Found {} enums", visitor.enums.len());
    Ok(())
}
