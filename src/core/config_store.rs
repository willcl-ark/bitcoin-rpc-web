use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::core::rpc_client::RpcConfig;

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            path: default_config_path()?,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> io::Result<RpcConfig> {
        if !self.path.exists() {
            return Ok(RpcConfig::default());
        }

        let bytes = fs::read(&self.path)?;
        serde_json::from_slice::<RpcConfig>(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, config: &RpcConfig) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let bytes = serde_json::to_vec_pretty(config)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        atomic_write_secure(&self.path, &bytes)
    }
}

fn atomic_write_secure(path: &Path, data: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;

    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp_path = parent.join(format!(".{file_name}.tmp"));

    {
        let mut file = create_secure_file(&tmp_path)?;
        file.write_all(data)?;
        file.sync_all()?;
    }

    fs::rename(&tmp_path, path)
}

#[cfg(unix)]
fn create_secure_file(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_secure_file(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
}

fn default_config_path() -> io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
        return Ok(home
            .join("Library")
            .join("Application Support")
            .join("bitcoin-rpc-web")
            .join("config.json"));
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(xdg)
                .join("bitcoin-rpc-web")
                .join("config.json"));
        }

        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
        Ok(home
            .join(".config")
            .join("bitcoin-rpc-web")
            .join("config.json"))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::core::config_store::ConfigStore;
    use crate::core::rpc_client::RpcConfig;

    #[test]
    fn roundtrip_load_save_preserves_values() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time must move forward")
            .as_nanos();

        let path = std::env::temp_dir().join(format!("bitcoin-rpc-web-config-{unique}.json"));
        let store = ConfigStore { path: path.clone() };

        let config = RpcConfig {
            url: "http://127.0.0.1:18443".to_string(),
            user: "alice".to_string(),
            password: "secret".to_string(),
            wallet: "hot".to_string(),
            poll_interval_secs: 11,
            zmq_address: "tcp://127.0.0.1:29000".to_string(),
            zmq_buffer_limit: 2048,
            font_size: 14,
            start_audio_playing: false,
        };

        store.save(&config).expect("config should save");
        let loaded = store.load().expect("config should load");

        assert_eq!(loaded, config);

        cleanup(path);
    }

    #[cfg(unix)]
    #[test]
    fn save_creates_file_with_mode_600() {
        use std::os::unix::fs::MetadataExt;

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time must move forward")
            .as_nanos();

        let path = std::env::temp_dir().join(format!("bitcoin-rpc-web-perms-{unique}.json"));
        let store = ConfigStore { path: path.clone() };

        store
            .save(&RpcConfig::default())
            .expect("config should save");

        let mode = std::fs::metadata(&path).expect("file should exist").mode() & 0o777;
        assert_eq!(mode, 0o600, "config file should be owner-only");

        cleanup(path);
    }

    fn cleanup(path: PathBuf) {
        let _ = std::fs::remove_file(path);
    }
}
