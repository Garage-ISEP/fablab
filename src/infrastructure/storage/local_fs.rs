use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::fs;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

use crate::application::errors::AppError;
use crate::domain::errors::DomainError;
use crate::domain::repositories::FileStorage;

use super::upload::{FileKind, UploadConfig, validate_magic_bytes};

/// File storage rooted at a canonicalized base directory. All writes
/// go through a .part temp file and are renamed atomically once fully
/// validated.
pub struct LocalFileStorage
{
    root: Arc<PathBuf>,
    config: UploadConfig,
}

/// Result of a successful upload: the canonical stored filename and the
/// number of bytes written to disk.
#[derive(Debug)]
pub struct StoredUpload
{
    pub stored_filename: String,
    pub size_bytes: u64,
}

impl LocalFileStorage
{
    /// Open or create the upload directory, set permissions to 0o700,
    /// canonicalize the path so that no symlink trick can escape the
    /// root later, and sweep any leftover .part files from a crashed
    /// previous run.
    pub async fn initialize(raw_path: &str, config: UploadConfig) -> Result<Self, DomainError>
    {
        let path = PathBuf::from(raw_path);

        fs::create_dir_all(&path).await
            .map_err(|e| DomainError::Database(format!("create upload dir: {e}")))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path).await
                .map_err(|e| DomainError::Database(format!("stat upload dir: {e}")))?
                .permissions();
            perms.set_mode(0o700);
            fs::set_permissions(&path, perms).await
                .map_err(|e| DomainError::Database(format!("chmod upload dir: {e}")))?;
        }

        let canonical = fs::canonicalize(&path).await
            .map_err(|e| DomainError::Database(format!("canonicalize upload dir: {e}")))?;

        sweep_partial_files(&canonical).await?;

        Ok(Self
        {
            root: Arc::new(canonical),
            config,
        })
    }

    pub fn config(&self) -> UploadConfig
    {
        self.config
    }

    /// Resolve a stored filename to a real path under the root, with a
    /// canonicalization guard. Returns NotFound if the file is missing
    /// or escapes the root.
    pub async fn resolve(&self, stored_filename: &str) -> Result<PathBuf, AppError>
    {
        if !is_safe_stored_name(stored_filename)
        {
            return Err(AppError::NotFound("file".to_owned()));
        }

        let candidate = self.root.join(stored_filename);
        let canonical = match fs::canonicalize(&candidate).await
        {
            Ok(p) => p,
            Err(_) => return Err(AppError::NotFound("file".to_owned())),
        };

        if !canonical.starts_with(self.root.as_path())
        {
            return Err(AppError::NotFound("file".to_owned()));
        }

        Ok(canonical)
    }

    /// Stream bytes from an async reader to disk, enforcing the size
    /// limit on the fly, then verify magic bytes once the complete file
    /// is available. Returns the canonical stored filename on success.
    /// On any error, the partial file is deleted.
    pub async fn store_upload<R>(
        &self,
        reader: &mut R,
        kind: FileKind,
    ) -> Result<StoredUpload, AppError>
    where
        R: AsyncRead + Unpin + Send,
    {
        let uuid = uuid::Uuid::new_v4();
        let stored_name = format!("{}.{}", uuid, kind.canonical_extension());
        let final_path = self.root.join(&stored_name);
        let tmp_path = self.root.join(format!("{stored_name}.part"));

        let mut file = match fs::File::create(&tmp_path).await
        {
            Ok(f) => f,
            Err(e) =>
            {
                eprintln!("upload: tmp create failed: {e}");
                return Err(AppError::Database("storage error".to_owned()));
            }
        };

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            if let Err(e) = fs::set_permissions(&tmp_path, perms).await
            {
                eprintln!("upload: chmod tmp failed: {e}");
                let _ = fs::remove_file(&tmp_path).await;
                return Err(AppError::Database("storage error".to_owned()));
            }
        }

        let mut total: u64 = 0;
        let max = self.config.max_upload_bytes;
        let mut buf = vec![0u8; 64 * 1024];
        let mut head: Vec<u8> = Vec::with_capacity(1024);

        loop
        {
            let n = match reader.read(&mut buf).await
            {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) =>
                {
                    eprintln!("upload: read error: {e}");
                    let _ = fs::remove_file(&tmp_path).await;
                    return Err(AppError::InvalidInput("upload interrupted".to_owned()));
                }
            };

            total = match total.checked_add(u64::try_from(n).unwrap_or(u64::MAX))
            {
                Some(v) => v,
                None =>
                {
                    let _ = fs::remove_file(&tmp_path).await;
                    return Err(AppError::InvalidInput("file too large".to_owned()));
                }
            };

            if total > max
            {
                let _ = fs::remove_file(&tmp_path).await;
                return Err(AppError::InvalidInput("file too large".to_owned()));
            }

            if head.len() < 1024
            {
                let take = (1024 - head.len()).min(n);
                head.extend_from_slice(&buf[..take]);
            }

            if let Err(e) = file.write_all(&buf[..n]).await
            {
                eprintln!("upload: write error: {e}");
                let _ = fs::remove_file(&tmp_path).await;
                return Err(AppError::Database("storage error".to_owned()));
            }
        }

        if let Err(e) = file.flush().await
        {
            eprintln!("upload: flush error: {e}");
            let _ = fs::remove_file(&tmp_path).await;
            return Err(AppError::Database("storage error".to_owned()));
        }
        drop(file);

        if total == 0
        {
            let _ = fs::remove_file(&tmp_path).await;
            return Err(AppError::InvalidInput("empty file".to_owned()));
        }

        if let Err(e) = validate_magic_bytes(kind, &head, total)
        {
            let _ = fs::remove_file(&tmp_path).await;
            return Err(e);
        }

        if let Err(e) = fs::rename(&tmp_path, &final_path).await
        {
            eprintln!("upload: rename error: {e}");
            let _ = fs::remove_file(&tmp_path).await;
            return Err(AppError::Database("storage error".to_owned()));
        }

        Ok(StoredUpload { stored_filename: stored_name, size_bytes: total })
    }

    pub async fn open_for_read(&self, stored_filename: &str) -> Result<fs::File, AppError>
    {
        let path = self.resolve(stored_filename).await?;
        fs::File::open(path).await
            .map_err(|_| AppError::NotFound("file".to_owned()))
    }
}

#[async_trait::async_trait]
impl FileStorage for LocalFileStorage
{
    async fn delete(&self, stored_filename: &str) -> Result<(), DomainError>
    {
        if !is_safe_stored_name(stored_filename)
        {
            return Ok(());
        }

        let path = self.root.join(stored_filename);
        match fs::remove_file(&path).await
        {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(DomainError::Database(format!("delete file: {e}"))),
        }
    }
}

/// A stored filename must be a uuid-shaped string followed by one of
/// the known canonical extensions. Anything else is refused.
pub fn is_safe_stored_name(name: &str) -> bool
{
    if name.len() < 36 + 4 || name.len() > 36 + 5
    {
        return false;
    }
    let (uuid_part, ext_part) = match name.rfind('.')
    {
        Some(i) => (&name[..i], &name[i + 1..]),
        None => return false,
    };
    if uuid::Uuid::parse_str(uuid_part).is_err()
    {
        return false;
    }
    matches!(ext_part, "stl" | "3mf" | "stp" | "step")
}

async fn sweep_partial_files(root: &Path) -> Result<(), DomainError>
{
    let mut entries = fs::read_dir(root).await
        .map_err(|e| DomainError::Database(format!("read upload dir: {e}")))?;

    while let Some(entry) = entries.next_entry().await
        .map_err(|e| DomainError::Database(format!("read entry: {e}")))?
    {
        let name = entry.file_name();
        let name_str = match name.to_str()
        {
            Some(s) => s,
            None => continue,
        };
        if name_str.ends_with(".part")
        {
            let _ = fs::remove_file(entry.path()).await;
        }
    }

    Ok(())
}

/// Read the entire content of a stored file into memory. Used for small
/// files in tests. Production download paths must stream.
#[cfg(test)]
pub async fn read_all(storage: &LocalFileStorage, name: &str) -> Vec<u8>
{
    let mut file = storage.open_for_read(name).await.unwrap();
    let mut out = Vec::new();
    file.read_to_end(&mut out).await.unwrap();
    out
}

#[cfg(test)]
mod tests
{
    use super::*;

    fn tmp_dir() -> PathBuf
    {
        let base = std::env::temp_dir().join(format!(
            "fablab-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    fn test_config() -> UploadConfig
    {
        UploadConfig
        {
            max_upload_bytes: 1024 * 1024,
            max_files_per_order: 10,
            max_total_storage_bytes: 100 * 1024 * 1024,
        }
    }

    fn ascii_stl_bytes() -> Vec<u8>
    {
        b"solid cube\nfacet normal 0 0 0\nendfacet\nendsolid\n".to_vec()
    }

    #[tokio::test]
    async fn is_safe_stored_name_accepts_uuid_stl()
    {
        let name = format!("{}.stl", uuid::Uuid::new_v4());
        assert!(is_safe_stored_name(&name));
    }

    #[tokio::test]
    async fn is_safe_stored_name_rejects_traversal()
    {
        assert!(!is_safe_stored_name("../etc/passwd"));
        assert!(!is_safe_stored_name("notauuid.stl"));
        assert!(!is_safe_stored_name("a.exe"));
    }

    #[tokio::test]
    async fn store_upload_writes_valid_stl()
    {
        let dir = tmp_dir();
        let storage = LocalFileStorage::initialize(dir.to_str().unwrap(), test_config())
            .await.unwrap();

        let data = ascii_stl_bytes();
        let mut reader = std::io::Cursor::new(data.clone());
        let result = storage.store_upload(&mut reader, FileKind::Stl).await.unwrap();
        assert_eq!(result.size_bytes, data.len() as u64);
        assert!(is_safe_stored_name(&result.stored_filename));

        let back = read_all(&storage, &result.stored_filename).await;
        assert_eq!(back, data);
    }

    #[tokio::test]
    async fn store_upload_rejects_oversize()
    {
        let dir = tmp_dir();
        let mut cfg = test_config();
        cfg.max_upload_bytes = 16;
        let storage = LocalFileStorage::initialize(dir.to_str().unwrap(), cfg)
            .await.unwrap();

        let data = vec![b'a'; 1024];
        let mut reader = std::io::Cursor::new(data);
        let err = storage.store_upload(&mut reader, FileKind::Stl).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));

        let mut entries = fs::read_dir(&*storage.root).await.unwrap();
        let mut count = 0;
        while (entries.next_entry().await.unwrap()).is_some()
        {
            count += 1;
        }
        assert_eq!(count, 0, "no file should remain after rejected upload");
    }

    #[tokio::test]
    async fn store_upload_rejects_bad_magic()
    {
        let dir = tmp_dir();
        let storage = LocalFileStorage::initialize(dir.to_str().unwrap(), test_config())
            .await.unwrap();

        let data = b"MZ this is not an STL".to_vec();
        let mut reader = std::io::Cursor::new(data);
        let err = storage.store_upload(&mut reader, FileKind::Stl).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn store_upload_rejects_empty()
    {
        let dir = tmp_dir();
        let storage = LocalFileStorage::initialize(dir.to_str().unwrap(), test_config())
            .await.unwrap();

        let mut reader = std::io::Cursor::new(Vec::<u8>::new());
        let err = storage.store_upload(&mut reader, FileKind::Stl).await.unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn delete_is_idempotent()
    {
        let dir = tmp_dir();
        let storage = LocalFileStorage::initialize(dir.to_str().unwrap(), test_config())
            .await.unwrap();

        let fake = format!("{}.stl", uuid::Uuid::new_v4());
        assert!(storage.delete(&fake).await.is_ok());
        assert!(storage.delete(&fake).await.is_ok());
    }

    #[tokio::test]
    async fn resolve_rejects_path_traversal()
    {
        let dir = tmp_dir();
        let storage = LocalFileStorage::initialize(dir.to_str().unwrap(), test_config())
            .await.unwrap();

        let err = storage.resolve("../outside.stl").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn sweep_removes_partial_files_on_init()
    {
        let dir = tmp_dir();
        let leftover = dir.join(format!("{}.stl.part", uuid::Uuid::new_v4()));
        fs::write(&leftover, b"junk").await.unwrap();

        let _ = LocalFileStorage::initialize(dir.to_str().unwrap(), test_config())
            .await.unwrap();

        assert!(!leftover.exists());
    }
}
