use handlebars::Handlebars;
use serde_json::Value;

use crate::error::WebhookError;

pub struct TemplateEngine {
    hbs: Handlebars<'static>,
}

impl TemplateEngine {
    pub fn new() -> Self {
        let mut hbs = Handlebars::new();
        hbs.set_strict_mode(false);
        Self { hbs }
    }

    pub fn register(
        &mut self,
        name: &str,
        template: &str,
    ) -> Result<(), handlebars::TemplateError> {
        self.hbs.register_template_string(name, template)
    }

    pub fn render(&self, name: &str, data: &Value) -> Result<String, WebhookError> {
        if !self.hbs.has_template(name) {
            return Err(WebhookError::TemplateNotFound(name.to_owned()));
        }
        self.hbs.render(name, data).map_err(WebhookError::from)
    }

    pub fn has_template(&self, name: &str) -> bool {
        self.hbs.has_template(name)
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}
