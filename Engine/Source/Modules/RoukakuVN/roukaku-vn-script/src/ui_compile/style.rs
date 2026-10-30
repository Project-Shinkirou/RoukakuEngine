use super::*;

/// Validated author style lowered to runtime theme tokens and deterministic selector rules.
#[derive(Debug, Clone)]
pub struct CompiledUiStyle {
    pub theme: UiThemeManifest,
    rules: BTreeMap<String, BTreeMap<String, UiValueExpr>>,
}

/// Read a `.style.jsonc` document using the published schema. No filesystem access.
pub fn compile_ui_style(path: &str, text: &str) -> Result<CompiledUiStyle, VnError> {
    if !path.ends_with(".style.jsonc") {
        return Err(VnError::diagnostic(
            "ROUKAKU_UI_STYLE_FORMAT",
            "UI styles must use .style.jsonc",
        ));
    }
    let doc = Document::parse(path, text, true)?;
    let tokens = serde_json::from_value(doc.value[":root"].clone())
        .map_err(|error| doc.error("/:root", "ROUKAKU_UI_THEME_VALUE", &error.to_string()))?;
    let high_contrast_tokens = match doc.value.get(":high-contrast") {
        Some(value) => serde_json::from_value(value.clone()).map_err(|error| {
            doc.error(
                "/:high-contrast",
                "ROUKAKU_UI_THEME_VALUE",
                &error.to_string(),
            )
        })?,
        None => BTreeMap::new(),
    };
    let theme = UiThemeManifest {
        schema: "roukaku.ui_theme_manifest.v1".to_string(),
        id: string(&doc.value, "$id").to_string(),
        parent: doc
            .value
            .get("$parent")
            .and_then(Value::as_str)
            .map(str::to_string),
        tokens,
        high_contrast_tokens,
        revision: 1,
    };
    theme
        .validate()
        .map_err(ui_error)
        .map_err(|error| doc.attach("", error))?;
    let mut rules = BTreeMap::new();
    for (selector, properties) in doc.value.as_object().expect("validated style") {
        if selector.starts_with('$') || selector.starts_with(':') {
            continue;
        }
        let mut rule = BTreeMap::new();
        for (key, value) in properties.as_object().expect("validated rule") {
            rule.insert(
                key.clone(),
                parse_expr(&doc, value, &pointer(&pointer("", selector), key))?,
            );
        }
        rules.insert(selector.clone(), rule);
    }
    Ok(CompiledUiStyle { theme, rules })
}

impl CompiledUiStyle {
    pub(super) fn apply(&self, node: &Value, properties: &mut BTreeMap<String, UiValueExpr>) {
        // Increasing specificity: kind, author-listed classes, identity, then inline properties.
        if let Some(rule) = self.rules.get(string(node, "kind")) {
            properties.extend(rule.clone());
        }
        if let Some(classes) = node.get("style").and_then(Value::as_str) {
            for class in classes.split_whitespace() {
                if let Some(rule) = self
                    .rules
                    .get(&format!(".{}", class.trim_start_matches('.')))
                {
                    properties.extend(rule.clone());
                }
            }
        }
        if let Some(rule) = self.rules.get(&format!("#{}", string(node, "id"))) {
            properties.extend(rule.clone());
        }
    }
}
