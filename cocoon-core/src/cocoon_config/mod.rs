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
    pub kv_database_config: KVDatabaseConfig,
    pub sqlite_config: SqliteConfig,
    pub working_directory: PathBuf,
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

        //create working directory if not exist
        let working_dir = PathBuf::from(config.get_str("working_directory")?);
        assert!(!working_dir.is_file()); //reject file path
        if !working_dir.is_dir() {
            std::fs::create_dir(working_dir).unwrap();
        }

        config.try_into()
    }
}
