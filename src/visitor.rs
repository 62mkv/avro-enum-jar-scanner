use anyhow::anyhow;
use noak::AccessFlags;
use noak::reader::{Attribute, AttributeContent, Class, cpool, Field};
use serde::Serialize;
use std::fmt::{Display, Formatter};
use std::fmt;

#[derive(Serialize)]
pub enum ClassSource {
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

#[derive(Serialize)]
pub struct EnumVisited {
    class_name: String,
    members: Vec<String>,
    avro_generated: bool,
    source: ClassSource
}

#[derive(Serialize)]
pub struct EnumVisitor {
    pub enums: Vec<EnumVisited>
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
