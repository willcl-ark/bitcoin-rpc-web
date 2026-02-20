use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodMeta {
    pub name: String,
    pub category: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SchemaIndex {
    methods: Vec<MethodMeta>,
}

impl SchemaIndex {
    pub fn load_default() -> Result<Self, String> {
        Self::from_str(include_str!("../../assets/openrpc.json"))
    }

    pub fn from_str(input: &str) -> Result<Self, String> {
        let root: Value =
            serde_json::from_str(input).map_err(|e| format!("invalid schema: {e}"))?;
        let methods = root["methods"]
            .as_array()
            .ok_or_else(|| "schema is missing methods array".to_string())?
            .iter()
            .filter_map(method_meta)
            .collect();

        Ok(Self { methods })
    }

    pub fn methods(&self) -> &[MethodMeta] {
        &self.methods
    }

    pub fn search(&self, query: &str) -> Vec<&MethodMeta> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self.methods.iter().collect();
        }

        self.methods
            .iter()
            .filter(|method| {
                method.name.to_lowercase().contains(&q)
                    || method.category.to_lowercase().contains(&q)
                    || method
                        .summary
                        .as_ref()
                        .is_some_and(|summary| summary.to_lowercase().contains(&q))
            })
            .collect()
    }
}

fn method_meta(method: &Value) -> Option<MethodMeta> {
    let name = method["name"].as_str()?.to_string();
    let summary = method["summary"].as_str().map(ToOwned::to_owned);

    let category = method["tags"]
        .as_array()
        .and_then(|tags| tags.first())
        .and_then(|tag| {
            tag["name"]
                .as_str()
                .map(ToOwned::to_owned)
                .or_else(|| tag.as_str().map(ToOwned::to_owned))
        })
        .unwrap_or_else(|| "uncategorized".to_string());

    Some(MethodMeta {
        name,
        category,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::SchemaIndex;

    #[test]
    fn default_schema_has_methods() {
        let index = SchemaIndex::load_default().expect("schema should load");
        assert!(!index.methods().is_empty());
    }

    #[test]
    fn search_matches_method_names_and_categories() {
        let index = SchemaIndex::load_default().expect("schema should load");
        assert!(!index.search("mempool").is_empty());
        assert!(!index.search("wallet").is_empty());
    }
}
