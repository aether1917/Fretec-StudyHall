use std::fs;
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::types::{AccountProfile, FlowItem};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowStore {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub items: Vec<FlowItem>,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            id: new_device_id(),
            name: default_device_name(),
        }
    }
}

fn new_device_id() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|value| format!("{value:02x}")).collect()
}

fn default_device_name() -> String {
    if let Ok(name) = std::env::var("COMPUTERNAME") {
        if !name.trim().is_empty() {
            return name;
        }
    }
    if let Ok(name) = std::env::var("HOSTNAME") {
        if !name.trim().is_empty() {
            return name;
        }
    }
    "Fretec StudyHall".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BiliCookies {
    #[serde(default)]
    pub sessdata: String,
    #[serde(default)]
    pub bili_jct: String,
    #[serde(default)]
    pub dede_user_id: String,
    #[serde(default)]
    pub buvid3: String,
    #[serde(default)]
    pub buvid4: String,
}

impl BiliCookies {
    pub fn is_logged_in(&self) -> bool {
        !self.sessdata.is_empty()
    }

    pub fn header(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !self.sessdata.is_empty() {
            parts.push(format!("SESSDATA={}", self.sessdata));
        }
        if !self.bili_jct.is_empty() {
            parts.push(format!("bili_jct={}", self.bili_jct));
        }
        if !self.dede_user_id.is_empty() {
            parts.push(format!("DedeUserID={}", self.dede_user_id));
        }
        if !self.buvid3.is_empty() {
            parts.push(format!("buvid3={}", self.buvid3));
        }
        if !self.buvid4.is_empty() {
            parts.push(format!("buvid4={}", self.buvid4));
        }
        parts.join("; ")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeteaseCookies {
    #[serde(default)]
    pub music_u: String,
    #[serde(default)]
    pub csrf: String,
}

impl NeteaseCookies {
    pub fn header(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !self.music_u.is_empty() {
            parts.push(format!("MUSIC_U={}", self.music_u));
        }
        if !self.csrf.is_empty() {
            parts.push(format!("__csrf={}", self.csrf));
        }
        parts.join("; ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BiliAuth {
    pub cookies: BiliCookies,
    pub profile: AccountProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeteaseAuth {
    pub cookies: NeteaseCookies,
    pub profile: AccountProfile,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStore {
    #[serde(default)]
    pub bili: Option<BiliAuth>,
    #[serde(default)]
    pub netease: Option<NeteaseAuth>,
}

#[derive(Clone)]
pub struct AppPaths {
    pub root: PathBuf,
}

impl AppPaths {
    pub fn new(root: PathBuf) -> Self {
        let _ = fs::create_dir_all(&root);
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn load_flow(&self) -> FlowStore {
        read_json(self.path("flow.json"))
    }

    pub fn save_flow(&self, store: &FlowStore) -> std::io::Result<()> {
        write_json(self.path("flow.json"), store)
    }

    pub fn load_device(&self) -> DeviceInfo {
        read_json(self.path("device.json"))
    }

    pub fn save_device(&self, device: &DeviceInfo) -> std::io::Result<()> {
        write_json(self.path("device.json"), device)
    }

    pub fn load_auth(&self) -> AuthStore {
        read_json(self.path("auth.json"))
    }

    pub fn save_auth(&self, store: &AuthStore) -> std::io::Result<()> {
        write_json(self.path("auth.json"), store)
    }
}

fn read_json<T: DeserializeOwned + Default>(path: PathBuf) -> T {
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => T::default(),
    }
}

fn write_json<T: Serialize>(path: PathBuf, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, data)?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    fs::rename(tmp, path)
}
