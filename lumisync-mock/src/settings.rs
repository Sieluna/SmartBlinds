use std::error::Error;
use std::path::PathBuf;
use std::{env, io};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logger {
    pub level: String,
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
    },
    Http {
        url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct External {
    pub default: Source,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mock {
    pub group_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub logger: Logger,
    pub external: External,
    pub mock: Mock,
}

impl Settings {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let mut settings: Settings = toml::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../",
            "configs/default.toml"
        )))?;

        if let Source::Mqtt {
            auth:
                Some(MqttAuth::TLSAuth {
                    cert_path,
                    key_path,
                }),
            ..
        } = &mut settings.external.default
        {
            *cert_path = Self::normalize_path(cert_path)?
                .to_string_lossy()
                .to_string();
            *key_path = Self::normalize_path(key_path)?
                .to_string_lossy()
                .to_string();
        }

        Ok(settings)
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
