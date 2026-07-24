use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use arc_swap::ArcSwap;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use serde::Deserialize;

use crate::rules::conditions::normalize_operator;

pub type RegexCache = HashMap<String, Regex>;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub global: Global,
    #[serde(default)]
    pub directory_listing: DirectoryListing,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(skip)]
    pub regexes: Arc<RegexCache>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Global {
    #[serde(default)]
    pub port: Option<u16>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DirectoryListing {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub conditions: Vec<ConditionNode>,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub status: Option<u16>,
    #[serde(default)]
    pub access: Option<Access>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConditionNode {
    #[serde(default)]
    pub and: Option<Vec<ConditionNode>>,
    #[serde(default)]
    pub or: Option<Vec<ConditionNode>>,
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub operator: Option<String>,
    #[serde(default)]
    pub value: Option<Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Access {
    #[serde(default)]
    pub bearer: Vec<String>,
    #[serde(default)]
    pub header: Vec<ConditionNode>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Value {
    List(Vec<String>),
    Text(String),
}

pub fn load() -> Config {
    let data = match std::fs::read_to_string("config.yaml") {
        Ok(data) => data,
        Err(error) => {
            tracing::warn!(error = %error, "Failed to load config.yaml, using defaults [config_load_failed]");
            return Config::default();
        }
    };
    let mut config = match serde_yaml::from_str::<Config>(&data) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(error = %error, "Failed to parse config.yaml, using defaults [config_parse_failed]");
            return Config::default();
        }
    };
    let regexes = build_regex_cache(&config.rules);
    let memory = estimate_memory(&config.rules, &regexes);
    tracing::info!(rules = config.rules.len(), patterns = regexes.len(), memory = %format_memory(memory), "Cached rules in memory [rules_cached]");
    config.regexes = Arc::new(regexes);
    config
}

fn build_regex_cache(rules: &[Rule]) -> RegexCache {
    let mut patterns = Vec::new();
    for rule in rules {
        collect_patterns(&rule.conditions, &mut patterns);
        if let Some(access) = &rule.access {
            collect_patterns(&access.header, &mut patterns);
        }
    }
    let mut cache = RegexCache::new();
    for pattern in patterns {
        if cache.contains_key(&pattern) {
            continue;
        }
        match Regex::new(&pattern) {
            Ok(regex) => {
                cache.insert(pattern, regex);
            }
            Err(error) => tracing::warn!(pattern = %pattern, error = %error, "Failed to compile rule pattern [rule_pattern_failed]"),
        }
    }
    cache
}

fn collect_patterns(nodes: &[ConditionNode], patterns: &mut Vec<String>) {
    for node in nodes {
        if let Some(children) = &node.and {
            collect_patterns(children, patterns);
        }
        if let Some(children) = &node.or {
            collect_patterns(children, patterns);
        }
        let is_matches = node.operator.as_ref().map(|operator| normalize_operator(operator) == "matches").unwrap_or(false);
        if let (true, Some(Value::Text(text))) = (is_matches, &node.value) {
            patterns.push(text.clone());
        }
    }
}

fn estimate_memory(rules: &[Rule], regexes: &RegexCache) -> usize {
    let structural: usize = rules.iter().map(estimate_rule).sum();
    let compiled: usize = regexes.keys().map(|pattern| pattern.len() + std::mem::size_of::<Regex>()).sum();
    structural + compiled
}

fn estimate_rule(rule: &Rule) -> usize {
    std::mem::size_of::<Rule>()
        + rule.name.len()
        + rule.action.len()
        + rule.message.as_ref().map(String::len).unwrap_or(0)
        + rule.conditions.iter().map(estimate_node).sum::<usize>()
        + rule.access.as_ref().map(estimate_access).unwrap_or(0)
}

fn estimate_node(node: &ConditionNode) -> usize {
    std::mem::size_of::<ConditionNode>()
        + node.field.as_ref().map(String::len).unwrap_or(0)
        + node.operator.as_ref().map(String::len).unwrap_or(0)
        + node.value.as_ref().map(estimate_value).unwrap_or(0)
        + node.and.as_ref().map(|children| children.iter().map(estimate_node).sum::<usize>()).unwrap_or(0)
        + node.or.as_ref().map(|children| children.iter().map(estimate_node).sum::<usize>()).unwrap_or(0)
}

fn estimate_value(value: &Value) -> usize {
    match value {
        Value::Text(text) => text.len(),
        Value::List(list) => list.iter().map(String::len).sum(),
    }
}

fn estimate_access(access: &Access) -> usize {
    access.bearer.iter().map(String::len).sum::<usize>() + access.header.iter().map(estimate_node).sum::<usize>()
}

fn format_memory(bytes: usize) -> String {
    let kilobytes = bytes as f64 / 1024.0;
    match kilobytes >= 1024.0 {
        true => format!("{:.2} MB", kilobytes / 1024.0),
        false => format!("{:.2} KB", kilobytes),
    }
}

pub fn watch(store: Arc<ArcSwap<Config>>) -> notify::Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| match event {
        Ok(change) => match change.kind {
            EventKind::Modify(_) | EventKind::Create(_) => {
                tracing::info!("Config file changed, reloading... [config_reload]");
                store.store(Arc::new(load()));
                tracing::info!("Config reloaded successfully [config_reloaded]");
            }
            _ => {}
        },
        Err(error) => {
            tracing::warn!(error = %error, "Config watcher error [config_watch_failed]")
        }
    })?;
    watcher.watch(Path::new("config.yaml"), RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
