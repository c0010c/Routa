//! AI 应用列表：内置兜底、远程镜像拉取、本地缓存。

pub mod catalog;
pub mod fetch;

use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub use catalog::{bundled, parse_catalog, Catalog, CatalogApp, InstallSource};

pub struct AiApps {
    catalog: RwLock<Catalog>,
    cache_path: PathBuf,
}

impl AiApps {
    /// 初始值 = choose(None, 缓存文件内容)。
    pub fn new(cache_path: PathBuf) -> Self {
        let cached = std::fs::read_to_string(&cache_path).ok();
        let (c, _) = fetch::choose(None, cached.as_deref());
        Self { catalog: RwLock::new(c), cache_path }
    }

    pub fn catalog(&self) -> Catalog {
        self.catalog.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set(&self, c: Catalog) {
        *self.catalog.write().unwrap_or_else(|e| e.into_inner()) = c;
    }

    pub fn cache_path(&self) -> &Path {
        &self.cache_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn missing() -> PathBuf {
        std::env::temp_dir().join("routa-aiapps-no-such-file.json")
    }

    #[test]
    fn new_falls_back_to_bundled_without_cache() {
        assert_eq!(AiApps::new(missing()).catalog(), bundled());
    }

    #[test]
    fn set_replaces_catalog() {
        let a = AiApps::new(missing());
        let mut c = bundled();
        c.apps.truncate(1);
        a.set(c.clone());
        assert_eq!(a.catalog(), c);
    }
}
