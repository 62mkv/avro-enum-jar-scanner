use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::PathBuf;

use clap::Parser;
use noak::AccessFlags;
use noak::reader::Class;
use regex::Regex;
use serde_json::to_string;
use zip::read::ZipFile;

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
    class_name_regex: Option<Regex>,
}

fn list_jars_in_classpath_order(classpath_index_file: ZipFile) -> anyhow::Result<Vec<String>> {
    let re = Regex::new(r#"- "(.+)""#).unwrap();
    Ok(io::BufReader::new(classpath_index_file)
        .lines()
        .map(|res| res.ok().unwrap())
        .map(|ref line| String::from(re.captures(line).unwrap().get(1).unwrap().as_str()))
        .collect())
}

fn list_zip_contents(reader: impl Read + Seek, source: &ClassSource, class_name_evaluator: &RegexEvaluator, enum_visitor: &mut EnumVisitor) -> anyhow::Result<()> {
    const CLASSPATH_INDEX_FILENAME: &str = "BOOT-INF/classpath.idx";

    let mut zip = zip::ZipArchive::new(reader)?;

    let classpath_index_found = zip.by_name(CLASSPATH_INDEX_FILENAME).ok().is_some();
    if matches!(source, ClassSource::Root) && classpath_index_found {
        let mut file_names: Vec<String> = Vec::with_capacity(100);

        let zip_iterator = zip.file_names();
        for file_name in zip_iterator {
            if file_name.starts_with("BOOT-INF/classes/") {
                file_names.push(String::from(file_name));
            }
        }

        for file_name in file_names {
            let mut file = zip.by_name(&file_name)?;
            process_file_from_zip(source, class_name_evaluator, enum_visitor, &mut file)?;
        }

        let classpath_index_file = zip.by_name(CLASSPATH_INDEX_FILENAME)?;
        let nested_jars = list_jars_in_classpath_order(classpath_index_file);
        for file_name in nested_jars? {
            match zip.by_name(&file_name) {
                Ok(mut file) => {
                    process_file_from_zip(source, class_name_evaluator, enum_visitor, &mut file)?;
                },
                Err(e) => {
                    eprintln!("{}", e);
                }
            }
        }
    } else {
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            process_file_from_zip(source, class_name_evaluator, enum_visitor, &mut file)?;
        }
    }

    Ok(())
}

fn process_file_from_zip(source: &ClassSource, class_name_evaluator: &RegexEvaluator, enum_visitor: &mut EnumVisitor, file: &mut ZipFile) -> anyhow::Result<()> {
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
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let jarfile = cli.jarfile;
    let jarfile = File::open(jarfile)?;


    let evaluator = RegexEvaluator::new(cli.class_name_regex);

    let mut visitor = EnumVisitor::new();
    list_zip_contents(jarfile, &ClassSource::Root, &evaluator, &mut visitor)?;

    println!("{}", to_string(&visitor)?);
    Ok(())
}
