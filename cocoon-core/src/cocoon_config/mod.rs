use config::{Config, ConfigError, Environment, File};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct KVDatabaseConfig {
    pub db_path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct SqliteConfig {
    pub db_path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct DaemonConfig {
    //  pub network_manager_config: NetworkManagerConfig,
    pub kv_database_config: KVDatabaseConfig,
    pub sqlite_config: SqliteConfig,
}

impl DaemonConfig {
    pub fn new(config_file_path: &Path) -> Result<Self, ConfigError> {
        if !config_file_path.exists() {
            panic!(
                "Config file does not exist!\n{}",
                &config_file_path.to_str().unwrap_or_default()
            );
        }
        if !config_file_path.is_file() {
            panic!(
                "{} is not a file!",
                &config_file_path.to_str().unwrap_or_default()
            )
        }
        let mut config = Config::default();
        config.merge(File::from(config_file_path).required(true))?;
        config.try_into()
    }
}
