use regex::Regex;

pub struct RegexEvaluator {
    class_name_regex: Option<Regex>
}

impl RegexEvaluator {
    pub fn new(class_name_regex: Option<Regex>) -> Self {
        RegexEvaluator {
            class_name_regex
        }
    }

    pub fn evaluate_if_class_needed(&self, class_name: &str) -> anyhow::Result<bool> {
        Ok(self.class_name_regex.as_ref().map(|r| r.is_match(class_name)).unwrap_or(false))
    }
}

