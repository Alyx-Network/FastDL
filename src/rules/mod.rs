pub mod actions;
pub mod conditions;

use axum::http::HeaderMap;

use crate::config::{RegexCache, Rule};
pub use actions::Decision;

pub struct Facts {
    pub path: String,
    pub user_agent: String,
    pub method: String,
    pub ext: String,
    pub ip: String,
    pub peer_ip: String,
}

pub fn process_rules(rules: &[Rule], regexes: &RegexCache, facts: &Facts, headers: &HeaderMap) -> Decision {
    for rule in rules {
        if !conditions::evaluate_all(&rule.conditions, regexes, facts, headers) {
            continue;
        }
        match actions::apply(rule, regexes, facts, headers) {
            Decision::Continue => continue,
            decision => return decision,
        }
    }
    Decision::Continue
}
