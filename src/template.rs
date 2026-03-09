use std::sync::{Arc, RwLock};

use handlebars::Handlebars;
use serde_json::Value;

use crate::error::WebhookError;

#[derive(Clone)]
pub struct TemplateEngine {
    hbs: Arc<RwLock<Handlebars<'static>>>,
}

impl TemplateEngine {
    pub fn new() -> Self {
        let mut hbs = Handlebars::new();
        hbs.set_strict_mode(false);
        Self {
            hbs: Arc::new(RwLock::new(hbs)),
        }
    }

    pub fn register(&self, name: &str, template: &str) -> Result<(), WebhookError> {
        self.hbs
            .write()
            .unwrap()
            .register_template_string(name, template)
            .map_err(WebhookError::from)
    }

    /// Overwrites an existing template or registers it if it doesn't exist yet.
    pub fn update(&self, name: &str, template: &str) -> Result<(), WebhookError> {
        self.register(name, template)
    }

    pub fn remove(&self, name: &str) {
        self.hbs.write().unwrap().unregister_template(name);
    }

    pub fn render(&self, name: &str, data: &Value) -> Result<String, WebhookError> {
        let hbs = self.hbs.read().unwrap();
        if !hbs.has_template(name) {
            return Err(WebhookError::TemplateNotFound(name.to_owned()));
        }
        hbs.render(name, data).map_err(WebhookError::from)
    }

    pub fn has_template(&self, name: &str) -> bool {
        self.hbs.read().unwrap().has_template(name)
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}
