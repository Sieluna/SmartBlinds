use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{env, error, fs, io};

use serde::{Deserialize, Serialize};
use toml::map::Map;
use toml::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logger {
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Database {
    pub migration_path: Option<String>,
    pub clean_start: bool,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Auth {
    pub secret: String,
    pub expiration: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MqttAuth {
    BasicAuth { username: String, password: String },
    TLSAuth { cert_path: String, key_path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Source {
    Mqtt {
        host: String,
        port: u16,
        client_id: String,
        topic: String,
        auth: Option<MqttAuth>,
        #[serde(default = "default_qos")]
        qos: u8,
        #[serde(default = "default_clean_session")]
        clean_session: bool,
        #[serde(default = "default_keep_alive")]
        keep_alive: u64,
    },
    Http {
        url: String,
    },
}

fn default_qos() -> u8 {
    0
}

fn default_clean_session() -> bool {
    true
}

fn default_keep_alive() -> u64 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedded {
    pub baud_rate: u32,
    pub port_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub server: Server,
    pub logger: Logger,
    pub database: Database,
    pub auth: Auth,
    pub external: HashMap<String, Source>,
    pub embedded: Option<Embedded>,
}

impl Settings {
    pub fn new() -> Result<Self, Box<dyn error::Error>> {
        let custom_config_path = env::var("CONFIG_PATH").ok();

        let mut settings: Settings = toml::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../",
            "configs/default.toml"
        )))?;

        let settings_path = custom_config_path
            .map(|path| Self::normalize_path(&path))
            .unwrap_or_else(|| {
                let run_mode = env::var("RUN_MODE").unwrap_or("development".into());
                Self::normalize_path(&format!("configs/{run_mode}.toml"))
            })?;

        if settings_path.exists() {
            let file_buffer = String::from_utf8(fs::read(&settings_path)?)?;
            let override_settings = Value::from_str(&file_buffer)?;
            let merged_settings = Self::merge(
                Value::try_from(settings.to_owned())?,
                override_settings,
                "$",
            )?;
            settings = Value::try_into(merged_settings)?;
        }

        for source in settings.external.values_mut() {
            if let Source::Mqtt {
                auth:
                    Some(MqttAuth::TLSAuth {
                        cert_path,
                        key_path,
                    }),
                ..
            } = source
            {
                *cert_path = Self::normalize_path(cert_path)?
                    .to_string_lossy()
                    .to_string();
                *key_path = Self::normalize_path(key_path)?
                    .to_string_lossy()
                    .to_string();
            }
        }

        if let Some(migrate) = &settings.database.migration_path {
            if Path::new(migrate).is_dir() {
                let migrate_path = Self::normalize_path(migrate)?.to_string_lossy().to_string();
                settings.database.migration_path = Some(migrate_path);
            } else {
                settings.database.migration_path = None;
            }
        }

        Ok(settings)
    }

    fn merge_table(
        value: &mut Map<String, Value>,
        other: Map<String, Value>,
        path: &str,
    ) -> Result<(), Box<dyn error::Error>> {
        for (name, inner) in other {
            if let Some(existing) = value.remove(&name) {
                let inner_path = format!("{path}.{name}");
                value.insert(name, Self::merge(existing, inner, &inner_path)?);
            } else {
                value.insert(name, inner);
            }
        }

        Ok(())
    }

    fn merge(value: Value, other: Value, path: &str) -> Result<Value, Box<dyn error::Error>> {
        match (value, other) {
            (Value::String(_), Value::String(inner)) => Ok(Value::String(inner)),
            (Value::Integer(_), Value::Integer(inner)) => Ok(Value::Integer(inner)),
            (Value::Float(_), Value::Float(inner)) => Ok(Value::Float(inner)),
            (Value::Boolean(_), Value::Boolean(inner)) => Ok(Value::Boolean(inner)),
            (Value::Datetime(_), Value::Datetime(inner)) => Ok(Value::Datetime(inner)),
            (Value::Array(mut existing), Value::Array(inner)) => {
                existing.extend(inner);
                Ok(Value::Array(existing))
            }
            (Value::Table(mut existing), Value::Table(inner)) => {
                Self::merge_table(&mut existing, inner, path)?;
                Ok(Value::Table(existing))
            }
            (v, o) => Err(format!(
                "Incompatible types at path {}, expected {} received {}.",
                path,
                v.type_str(),
                o.type_str()
            )
            .into()),
        }
    }

    fn normalize_path(path: &str) -> io::Result<PathBuf> {
        let path_buf = PathBuf::from(path);

        Ok(if path_buf.is_absolute() {
            path_buf.clone()
        } else {
            env::current_dir()?.as_path().join(&path_buf)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_default_config() {
        let settings = Settings::new().unwrap();

        assert!(!settings.server.host.is_empty());
        assert!(settings.server.port > 0);

        assert!(!settings.logger.level.is_empty());

        assert!(!settings.database.url.is_empty());

        assert!(!settings.auth.secret.is_empty());
        assert!(settings.auth.expiration > 0);

        assert!(!settings.external.is_empty());
        if let Source::Mqtt {
            host,
            port,
            client_id,
            topic,
            auth,
            qos,
            clean_session,
            keep_alive,
        } = settings.external.get("default").unwrap()
        {
            assert!(!host.is_empty());
            assert!(*port > 0);
            assert!(!client_id.is_empty());
            assert!(!topic.is_empty());
            assert!(*qos == 0);
            assert!(clean_session == &true || clean_session == &false);
            assert!(*keep_alive > 0);

            if let Some(auth_config) = auth {
                match auth_config {
                    MqttAuth::BasicAuth { username, password } => {
                        assert!(!username.is_empty());
                        assert!(!password.is_empty());
                    }
                    MqttAuth::TLSAuth {
                        cert_path,
                        key_path,
                    } => {
                        assert!(!cert_path.is_empty());
                        assert!(!key_path.is_empty());
                    }
                }
            }
        }
    }
}
