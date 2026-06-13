use axum::http::{header, HeaderMap};

use super::conditions::{evaluate_node, normalize_operator};
use super::Facts;
use crate::config::{Access, Rule};

pub enum Decision {
    Continue,
    Allow,
    Deny { status: u16, message: String, rule: String },
}

pub fn apply(rule: &Rule, facts: &Facts, headers: &HeaderMap) -> Decision {
    match normalize_operator(&rule.action).as_str() {
        "deny" | "block" => Decision::Deny {
            status: rule.status.unwrap_or(403),
            message: rule.message.clone().unwrap_or_else(|| "Forbidden".to_string()),
            rule: rule.name.clone(),
        },
        "allow" => Decision::Allow,
        "access" => match access_satisfied(rule.access.as_ref(), facts, headers) {
            true => Decision::Continue,
            false => Decision::Deny {
                status: rule.status.unwrap_or(401),
                message: rule.message.clone().unwrap_or_else(|| "Unauthorized".to_string()),
                rule: rule.name.clone(),
            },
        },
        _ => Decision::Continue,
    }
}

fn access_satisfied(access: Option<&Access>, facts: &Facts, headers: &HeaderMap) -> bool {
    let access = match access {
        Some(access) => access,
        None => return false,
    };
    bearer_satisfied(&access.bearer, headers)
        || access.header.iter().any(|node| evaluate_node(node, facts, headers))
}

fn bearer_satisfied(tokens: &[String], headers: &HeaderMap) -> bool {
    if tokens.is_empty() {
        return false;
    }
    let provided = match headers.get(header::AUTHORIZATION).and_then(|value| value.to_str().ok()) {
        Some(value) => value.strip_prefix("Bearer ").unwrap_or(value).trim(),
        None => return false,
    };
    tokens.iter().any(|token| token == provided)
}
