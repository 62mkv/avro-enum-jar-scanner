use anyhow::anyhow;
use noak::AccessFlags;
use noak::reader::{Attribute, AttributeContent, Class, cpool, Field};
use serde::{Serialize, Serializer};
use std::fmt::{Display, Formatter};
use std::fmt;

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

impl Serialize for ClassSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        match self {
            ClassSource::Root => {
                serializer.serialize_str("root")
            },
            ClassSource::NestedJar(ref name) => {
                serializer.serialize_str(name)
            }
        }
    }
}

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
        let class_name = class.this_class_name()?;
        let already_scanned = self.enums.iter().find(|e| e.class_name.eq(class_name.to_str().unwrap())).is_some();
        if already_scanned {
            eprintln!("Already scanned {}", class_name.display());
            return Ok(());
        }
        let internal_type_name = class.this_class_name()?.to_str().ok_or(anyhow!("Error decoding type name {}", class_name.display()))?;
        let internal_type_name = format!("L{};", internal_type_name);
        let mut enum_members: Vec<String> = Vec::new();
        for field in class.fields()? {
            let fld: &Field = &field?;
            if fld.access_flags().contains(ENUM_MEMBER_FLAGS) {
                let pool = class.pool()?;
                let field_name: &cpool::Utf8 = pool.get(fld.name())?;
                let descriptor: &cpool::Utf8 = pool.get(fld.descriptor())?;
                if internal_type_name.eq(descriptor.content.to_str().unwrap_or("")) {
                    enum_members.push(field_name.content.to_str().unwrap().to_string());
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
