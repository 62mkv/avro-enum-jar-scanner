use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::PathBuf;

use clap::Parser;
use noak::AccessFlags;
use noak::reader::Class;
use regex::Regex;
use serde_json::to_string;

use evaluator::RegexEvaluator;
use visitor::{ClassSource, EnumVisitor};

mod evaluator;
mod visitor;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// jar-file to process
    #[arg(short, long, value_name = "JAR_FILE")]
    jarfile: PathBuf,

    #[arg(short, long, value_name = "REGEX")]
    class_name_regex: Option<Regex>
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

    println!("Serialized view is: {}", to_string(&visitor)?);
    Ok(())
}
