use std::net::IpAddr;

use axum::http::HeaderMap;
use ipnet::IpNet;
use regex::Regex;

use super::Facts;
use crate::config::{ConditionNode, Value};

pub fn normalize_operator(operator: &str) -> String {
    operator.to_lowercase().replace('_', "")
}

pub fn evaluate_all(nodes: &[ConditionNode], facts: &Facts, headers: &HeaderMap) -> bool {
    nodes.iter().all(|node| evaluate_node(node, facts, headers))
}

pub fn evaluate_node(node: &ConditionNode, facts: &Facts, headers: &HeaderMap) -> bool {
    if let Some(children) = &node.and {
        return children.iter().all(|child| evaluate_node(child, facts, headers));
    }
    if let Some(children) = &node.or {
        return children.iter().any(|child| evaluate_node(child, facts, headers));
    }
    evaluate_leaf(node, facts, headers)
}

fn field_value(field: &str, facts: &Facts, headers: &HeaderMap) -> String {
    if field == "ip" || field == "client_ip" {
        return facts.ip.clone();
    }
    if field == "peer_ip" {
        return facts.peer_ip.clone();
    }
    if let Some(name) = field.strip_prefix("header:") {
        return headers
            .get(name.to_lowercase().as_str())
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
    }
    match field {
        "path" => facts.path.clone(),
        "userAgent" | "user_agent" => facts.user_agent.clone(),
        "method" => facts.method.clone(),
        "ext" => facts.ext.clone(),
        _ => String::new(),
    }
}

fn value_text(value: &Option<Value>) -> Option<&str> {
    match value {
        Some(Value::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

fn value_list(value: &Option<Value>) -> Option<&[String]> {
    match value {
        Some(Value::List(list)) => Some(list.as_slice()),
        _ => None,
    }
}

fn ip_matches(client: &str, pattern: &str) -> bool {
    let address = match client.parse::<IpAddr>() {
        Ok(address) => address,
        Err(_) => return client == pattern,
    };
    if let Ok(network) = pattern.parse::<IpNet>() {
        return network.contains(&address);
    }
    match pattern.parse::<IpAddr>() {
        Ok(other) => address == other,
        Err(_) => false,
    }
}

fn evaluate_leaf(node: &ConditionNode, facts: &Facts, headers: &HeaderMap) -> bool {
    let field = match &node.field {
        Some(field) => field.as_str(),
        None => return false,
    };
    let operator = match &node.operator {
        Some(operator) => normalize_operator(operator),
        None => return false,
    };
    let resolved = field_value(field, facts, headers);
    let is_ip = field == "ip" || field == "client_ip" || field == "peer_ip";
    match operator.as_str() {
        "equals" => match is_ip {
            true => value_text(&node.value).map(|text| ip_matches(&resolved, text)).unwrap_or(false),
            false => value_text(&node.value).map(|text| resolved == text).unwrap_or(false),
        },
        "notequals" => match is_ip {
            true => value_text(&node.value).map(|text| !ip_matches(&resolved, text)).unwrap_or(false),
            false => value_text(&node.value).map(|text| resolved != text).unwrap_or(false),
        },
        "startswith" => value_text(&node.value).map(|text| resolved.starts_with(text)).unwrap_or(false),
        "endswith" => value_text(&node.value).map(|text| resolved.ends_with(text)).unwrap_or(false),
        "contains" => value_text(&node.value).map(|text| resolved.contains(text)).unwrap_or(false),
        "matches" => value_text(&node.value)
            .and_then(|text| Regex::new(text).ok())
            .map(|regex| regex.is_match(&resolved))
            .unwrap_or(false),
        "in" => match is_ip {
            true => value_list(&node.value).map(|list| list.iter().any(|item| ip_matches(&resolved, item))).unwrap_or(false),
            false => value_list(&node.value).map(|list| list.iter().any(|item| item == &resolved)).unwrap_or(false),
        },
        "notin" => match is_ip {
            true => value_list(&node.value).map(|list| !list.iter().any(|item| ip_matches(&resolved, item))).unwrap_or(false),
            false => value_list(&node.value).map(|list| !list.iter().any(|item| item == &resolved)).unwrap_or(false),
        },
        "exists" => !resolved.is_empty(),
        "notexists" => resolved.is_empty(),
        _ => false,
    }
}
