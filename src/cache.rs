//! 磁碟快取：%LOCALAPPDATA%\tft-tool。資料用 bincode（讀取只要幾毫秒）。

use serde::{de::DeserializeOwned, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

pub fn dir() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let d = base.join("tft-tool");
        let _ = std::fs::create_dir_all(d.join("icons"));
        d
    })
}

pub fn load<T: DeserializeOwned>(name: &str) -> Option<T> {
    let bytes = std::fs::read(dir().join(name)).ok()?;
    bincode::deserialize(&bytes).ok()
}

/// 先寫暫存檔再改名，避免寫到一半當機留下壞檔。
pub fn save<T: Serialize>(name: &str, value: &T) {
    let Ok(bytes) = bincode::serialize(value) else { return };
    let path = dir().join(name);
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, bytes).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

pub fn stats_key(f: &crate::model::Filter) -> String {
    format!("stats_{}_{}_{}.bin", f.game_type, f.rank_group, f.patch)
}

pub fn comps_key(f: &crate::model::Filter) -> String {
    format!("comps_{}_{}_{}.bin", f.game_type, f.rank_group, f.patch)
}

pub fn entity_key(k: &crate::model::EntityKey) -> String {
    let f = &k.filter;
    let kind = if k.is_item { "item" } else { "unit" };
    format!("{kind}_{}_{}_{}_{}.bin", f.game_type, f.rank_group, f.patch, k.id)
}

pub fn icon_path(url: &str) -> PathBuf {
    // FNV-1a 雜湊當檔名
    let mut h: u64 = 0xcbf29ce484222325;
    for b in url.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    dir().join("icons").join(format!("{h:016x}.png"))
}
