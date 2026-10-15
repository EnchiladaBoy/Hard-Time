use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

pub const ASSET_DIRECTORIES: [&str; 6] = [
    "Characters",
    "Graphics",
    "Sound",
    "Weapons",
    "World",
    "Data",
];

/// Resolve original DOS-style asset names consistently on case-sensitive hosts too.
/// No guessed parent paths or asset-supplied absolute filenames are accepted.
#[derive(Debug, Clone)]
pub struct AssetRoot(PathBuf);

impl AssetRoot {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let root = Self(fs::canonicalize(path)?);
        for directory in ASSET_DIRECTORIES {
            if !root.resolve(directory)?.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Missing asset directory: {directory}"),
                ));
            }
        }
        Ok(root)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn resolve(&self, relative: impl AsRef<Path>) -> io::Result<PathBuf> {
        let relative = relative.as_ref();
        let mut current = self.0.clone();
        if relative.as_os_str().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Empty asset path",
            ));
        }
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Asset path must stay inside the game folder",
                ));
            };
            let direct = current.join(name);
            if direct.exists() {
                current = direct;
            } else {
                let mut matches = fs::read_dir(&current)?
                    .filter_map(Result::ok)
                    .filter(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(&name.to_string_lossy())
                    });
                let found = matches.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("Missing asset: {}", relative.display()),
                    )
                })?;
                if matches.next().is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Ambiguous asset filename",
                    ));
                }
                current = found.path();
            }
            current = fs::canonicalize(current)?;
            if !current.starts_with(&self.0) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Asset link points outside the game folder",
                ));
            }
        }
        Ok(current)
    }

    pub fn read(&self, relative: impl AsRef<Path>, max_bytes: u64) -> io::Result<Vec<u8>> {
        use io::Read;
        let path = self.resolve(relative)?;
        let file = fs::File::open(&path)?;
        if file.metadata()?.len() > max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Asset is too large: {}", path.display()),
            ));
        }
        let mut bytes = Vec::new();
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Asset grew beyond the size limit",
            ));
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_assets_support_unicode_case_and_reject_escape() {
        let path = std::env::temp_dir().join(format!("hardtime-assets-猫-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        for directory in ASSET_DIRECTORIES {
            fs::create_dir(path.join(directory)).unwrap();
        }
        fs::write(path.join("World/MODEL.3DS"), b"test").unwrap();
        let root = AssetRoot::open(&path).unwrap();
        assert_eq!(root.read("world/model.3ds", 4).unwrap(), b"test");
        assert!(root.read("world/model.3ds", 3).is_err());
        assert!(root.resolve("../secret").is_err());
        assert!(root.resolve(path.join("World/MODEL.3DS")).is_err());
        fs::remove_dir_all(&path).unwrap();
    }
}
