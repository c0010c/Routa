//! On-disk cache of a downloaded, signed installer. The directory is
//! user-writable while the installer may run elevated, so the bytes are
//! re-verified against the minisign signature every time before use.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::{Deserialize, Serialize};

const EXE: &str = "pending.exe";
const EXE_TMP: &str = "pending.exe.tmp";
const META: &str = "pending.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub version: String,
    /// Tauri-format signature (base64 of the full minisign signature text).
    pub signature: String,
    pub notes: Option<String>,
}

pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new(data_dir: &Path) -> Cache {
        Cache { dir: data_dir.join("update") }
    }

    pub fn installer_path(&self) -> PathBuf {
        self.dir.join(EXE)
    }

    pub fn write(&self, bytes: &[u8], pending: &Pending) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        // Drop stale metadata first so a half-written cache is never trusted.
        let _ = fs::remove_file(self.dir.join(META));
        let tmp = self.dir.join(EXE_TMP);
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, self.installer_path())?;
        let json = serde_json::to_vec(pending).map_err(io::Error::other)?;
        let mut f = fs::File::create(self.dir.join(META))?;
        f.write_all(&json)?;
        f.sync_all()
    }

    pub fn clear(&self) {
        let _ = fs::remove_dir_all(&self.dir);
    }

    pub fn load_verified(&self, pubkey_b64: &str, current: &semver::Version) -> Option<Pending> {
        let result = self.try_load(pubkey_b64, current);
        if result.is_none() {
            self.clear();
        }
        result
    }

    fn try_load(&self, pubkey_b64: &str, current: &semver::Version) -> Option<Pending> {
        let meta = fs::read(self.dir.join(META)).ok()?;
        let pending: Pending = serde_json::from_slice(&meta).ok()?;
        let version = semver::Version::parse(&pending.version).ok()?;
        if version <= *current {
            return None;
        }
        let bytes = fs::read(self.installer_path()).ok()?;
        verify(&bytes, &pending.signature, pubkey_b64, &pending.version).ok()?;
        Some(pending)
    }
}

fn decode_text(b64: &str, what: &str) -> Result<String, String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("{what} is not valid base64: {e}"))?;
    String::from_utf8(raw).map_err(|e| format!("{what} is not valid UTF-8: {e}"))
}

pub fn verify(
    bytes: &[u8],
    signature_b64: &str,
    pubkey_b64: &str,
    version: &str,
) -> Result<(), String> {
    let pk_text = decode_text(pubkey_b64, "public key")?;
    let sig_text = decode_text(signature_b64, "signature")?;
    let public_key = minisign_verify::PublicKey::decode(&pk_text)
        .map_err(|e| format!("invalid public key: {e}"))?;
    let signature = minisign_verify::Signature::decode(&sig_text)
        .map_err(|e| format!("invalid signature: {e}"))?;
    public_key
        .verify(bytes, &signature, true)
        .map_err(|e| format!("signature verification failed: {e}"))?;
    let file = signature
        .trusted_comment()
        .split('\t')
        .find_map(|part| part.strip_prefix("file:"))
        .ok_or_else(|| "signature trusted comment has no file name".to_string())?;
    if !file.contains(&format!("_{version}_")) {
        return Err(format!("signature is for '{file}', not version {version}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keypair_and_sign(bytes: &[u8], file: &str) -> (String, String) {
        let kp = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let comment = format!("timestamp:0\tfile:{file}");
        let sig = minisign::sign(
            Some(&kp.pk),
            &kp.sk,
            io::Cursor::new(bytes),
            Some(&comment),
            Some("test"),
        )
        .unwrap();
        let b64 = |s: String| base64::engine::general_purpose::STANDARD.encode(s);
        (b64(kp.pk.to_box().unwrap().into_string()), b64(sig.into_string()))
    }

    const BYTES: &[u8] = b"fake installer bytes";

    fn setup(file: &str, version: &str) -> (tempfile::TempDir, Cache, String) {
        let dir = tempfile::tempdir().unwrap();
        let (pk, sig) = keypair_and_sign(BYTES, file);
        let cache = Cache::new(dir.path());
        let pending = Pending { version: version.into(), signature: sig, notes: Some("n".into()) };
        cache.write(BYTES, &pending).unwrap();
        (dir, cache, pk)
    }

    fn v(s: &str) -> semver::Version {
        semver::Version::parse(s).unwrap()
    }

    #[test]
    fn verified_newer_cache_is_offered() {
        let (_d, cache, pk) = setup("routa_0.4.0_x64-setup.exe", "0.4.0");
        let p = cache.load_verified(&pk, &v("0.3.0")).unwrap();
        assert_eq!(p.version, "0.4.0");
        assert_eq!(p.notes.as_deref(), Some("n"));
        assert!(cache.installer_path().exists());
        assert!(!cache.dir.join(EXE_TMP).exists());
    }

    #[test]
    fn same_or_older_version_is_cleared() {
        for cur in ["0.4.0", "0.5.0"] {
            let (_d, cache, pk) = setup("routa_0.4.0_x64-setup.exe", "0.4.0");
            assert!(cache.load_verified(&pk, &v(cur)).is_none());
            assert!(!cache.dir.join(META).exists());
            assert!(!cache.installer_path().exists());
        }
    }

    #[test]
    fn tampered_bytes_are_cleared() {
        let (_d, cache, pk) = setup("routa_0.4.0_x64-setup.exe", "0.4.0");
        let mut b = fs::read(cache.installer_path()).unwrap();
        b[0] ^= 1;
        fs::write(cache.installer_path(), b).unwrap();
        assert!(cache.load_verified(&pk, &v("0.3.0")).is_none());
        assert!(!cache.dir.exists() || fs::read_dir(&cache.dir).unwrap().next().is_none());
    }

    #[test]
    fn signature_for_other_version_is_rejected() {
        let (_d, cache, pk) = setup("routa_0.3.9_x64-setup.exe", "0.4.0");
        assert!(cache.load_verified(&pk, &v("0.3.0")).is_none());
        assert!(!cache.installer_path().exists());
        assert!(!cache.dir.join(META).exists());
    }

    #[test]
    fn missing_or_corrupt_metadata_is_cleared() {
        let (_d, cache, pk) = setup("routa_0.4.0_x64-setup.exe", "0.4.0");
        fs::write(cache.dir.join(META), "{oops").unwrap();
        assert!(cache.load_verified(&pk, &v("0.3.0")).is_none());
        assert!(!cache.installer_path().exists());
        assert!(!cache.dir.join(META).exists());

        fs::create_dir_all(&cache.dir).unwrap();
        fs::write(cache.dir.join(EXE_TMP), b"partial").unwrap();
        assert!(cache.load_verified(&pk, &v("0.3.0")).is_none());
        assert!(!cache.dir.join(EXE_TMP).exists());
    }
}
