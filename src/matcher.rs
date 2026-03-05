use std::any::{Any, TypeId};
use std::collections::HashMap;

type RuleFn = Box<dyn Fn(&dyn Any) -> &'static str + Send + Sync>;

pub struct MatcherRegistry {
    rules: HashMap<TypeId, RuleFn>,
    default_template: &'static str,
}

impl MatcherRegistry {
    pub fn new(default_template: &'static str) -> Self {
        Self {
            rules: HashMap::new(),
            default_template,
        }
    }

    pub fn register<T: 'static>(
        &mut self,
        rule: impl Fn(&T) -> &'static str + Send + Sync + 'static,
    ) {
        let wrapped: RuleFn = Box::new(move |any| {
            let typed = any
                .downcast_ref::<T>()
                .expect("TypeId matched but downcast failed");
            rule(typed)
        });
        self.rules.insert(TypeId::of::<T>(), wrapped);
    }

    pub fn resolve<T: 'static>(&self, event: &T) -> &'static str {
        self.rules
            .get(&TypeId::of::<T>())
            .map(|rule| rule(event))
            .unwrap_or(self.default_template)
    }
}
