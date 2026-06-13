use std::path::Path;
use std::sync::Arc;

use arc_swap::ArcSwap;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub global: Global,
    #[serde(default)]
    pub directory_listing: DirectoryListing,
    #[serde(default)]
    pub rules: Vec<Rule>,
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
    match serde_yaml::from_str::<Config>(&data) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(error = %error, "Failed to parse config.yaml, using defaults [config_parse_failed]");
            Config::default()
        }
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
