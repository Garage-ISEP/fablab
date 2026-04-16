use std::env;

use crate::application::errors::AppError;

pub const DEFAULT_MAX_UPLOAD_BYTES: u64 = 50 * 1024 * 1024;
pub const DEFAULT_MAX_FILES_PER_ORDER: i64 = 10;
pub const DEFAULT_MAX_TOTAL_STORAGE_BYTES: u64 = 10 * 1024 * 1024 * 1024;

const MAGIC_SCAN_BYTES: usize = 1024;

/// Runtime configuration for uploads. Loaded once at startup from the
/// environment, then passed as an immutable value into use cases.
#[derive(Debug, Clone, Copy)]
pub struct UploadConfig
{
    pub max_upload_bytes: u64,
    pub max_files_per_order: i64,
    pub max_total_storage_bytes: u64,
}

impl UploadConfig
{
    pub fn from_env() -> Self
    {
        Self
        {
            max_upload_bytes: parse_u64_env("MAX_UPLOAD_BYTES", DEFAULT_MAX_UPLOAD_BYTES),
            max_files_per_order: parse_i64_env(
                "MAX_FILES_PER_ORDER",
                DEFAULT_MAX_FILES_PER_ORDER,
            ),
            max_total_storage_bytes: parse_u64_env(
                "MAX_TOTAL_STORAGE_BYTES",
                DEFAULT_MAX_TOTAL_STORAGE_BYTES,
            ),
        }
    }
}

fn parse_u64_env(key: &str, default: u64) -> u64
{
    env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn parse_i64_env(key: &str, default: i64) -> i64
{
    env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Supported file kinds. The extension carries semantic meaning because
/// we never parse the file; the extension and the header bytes must be
/// consistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind
{
    Stl,
    ThreeMf,
    Step,
}

impl FileKind
{
    /// Canonical lowercase extension written on disk.
    pub fn canonical_extension(self) -> &'static str
    {
        match self
        {
            FileKind::Stl => "stl",
            FileKind::ThreeMf => "3mf",
            FileKind::Step => "stp",
        }
    }

    /// Content type used only for DB storage. Responses always send
    /// application/octet-stream regardless of this value.
    pub fn mime_type(self) -> &'static str
    {
        match self
        {
            FileKind::Stl => "model/stl",
            FileKind::ThreeMf => "model/3mf",
            FileKind::Step => "model/step",
        }
    }
}

/// Extract a canonical file kind from the user-provided filename.
/// Rejects null bytes, path separators, leading dots, traversal
/// sequences, overly long names, and control characters.
/// Accepts unicode letters and digits (French names like "modele.stl"
/// or "piece-specifique.3mf" are allowed).
/// Never trusts the client Content-Type.
pub fn validate_filename(raw: &str) -> Result<(String, FileKind), AppError>
{
    let trimmed = raw.trim();

    if trimmed.is_empty() || trimmed.len() > 255
    {
        return Err(AppError::InvalidInput("invalid file name".to_owned()));
    }

    if trimmed.contains('\0')
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
    {
        return Err(AppError::InvalidInput("invalid file name".to_owned()));
    }

    if trimmed.starts_with('.')
    {
        return Err(AppError::InvalidInput("invalid file name".to_owned()));
    }

    // Reject any control character (including newline, tab, DEL) and
    // any character that could be interpreted as a path separator on
    // unusual filesystems. We allow letters and digits (any script,
    // including accented French), plus a small set of punctuation.
    let allowed_punct = [' ', '-', '_', '.', '(', ')', '[', ']', '\''];
    if !trimmed.chars().all(|c|
    {
        if c.is_control()
        {
            return false;
        }
        c.is_alphanumeric() || allowed_punct.contains(&c)
    })
    {
        return Err(AppError::InvalidInput("invalid file name".to_owned()));
    }

    let dot = trimmed.rfind('.')
        .ok_or_else(|| AppError::InvalidInput("missing file extension".to_owned()))?;
    if dot == 0 || dot == trimmed.len() - 1
    {
        return Err(AppError::InvalidInput("invalid file extension".to_owned()));
    }

    let ext = trimmed[dot + 1..].to_ascii_lowercase();
    let kind = match ext.as_str()
    {
        "stl" => FileKind::Stl,
        "3mf" => FileKind::ThreeMf,
        "stp" | "step" => FileKind::Step,
        _ => return Err(AppError::InvalidInput("unsupported file type".to_owned())),
    };

    Ok((trimmed.to_owned(), kind))
}

/// Validate the first bytes of a file against the expected kind.
/// Works on a prefix; the caller is responsible for passing at least
/// MAGIC_SCAN_BYTES bytes when available.
///
/// total_size is the final size on disk; for STL binary we also check
/// the triangle count formula, which catches truncated or forged files.
pub fn validate_magic_bytes(
    kind: FileKind,
    prefix: &[u8],
    total_size: u64,
) -> Result<(), AppError>
{
    match kind
    {
        FileKind::Stl =>
        {
            if is_stl_ascii(prefix)
            {
                return Ok(());
            }
            if is_stl_binary(prefix, total_size)
            {
                return Ok(());
            }
            Err(AppError::InvalidInput("file content does not match extension".to_owned()))
        }
        FileKind::ThreeMf =>
        {
            if prefix.starts_with(b"PK\x03\x04")
            {
                Ok(())
            }
            else
            {
                Err(AppError::InvalidInput("file content does not match extension".to_owned()))
            }
        }
        FileKind::Step =>
        {
            let scan = &prefix[..prefix.len().min(MAGIC_SCAN_BYTES)];
            if contains_subsequence(scan, b"ISO-10303")
            {
                Ok(())
            }
            else
            {
                Err(AppError::InvalidInput("file content does not match extension".to_owned()))
            }
        }
    }
}

fn is_stl_ascii(prefix: &[u8]) -> bool
{
    if !prefix.starts_with(b"solid ") && !prefix.starts_with(b"solid\t")
        && !prefix.starts_with(b"solid\n")
    {
        return false;
    }

    let scan = &prefix[..prefix.len().min(MAGIC_SCAN_BYTES)];
    scan.iter().all(|b| *b == b'\t' || *b == b'\n' || *b == b'\r' || (0x20..=0x7E).contains(b))
}

fn is_stl_binary(prefix: &[u8], total_size: u64) -> bool
{
    if prefix.len() < 84
    {
        return false;
    }
    if total_size < 84
    {
        return false;
    }

    let count_bytes: [u8; 4] = match prefix[80..84].try_into()
    {
        Ok(b) => b,
        Err(_) => return false,
    };
    let triangle_count = u32::from_le_bytes(count_bytes);

    if triangle_count > 10_000_000
    {
        return false;
    }

    let expected: u64 = 84 + 50 * u64::from(triangle_count);
    expected == total_size
}

fn contains_subsequence(haystack: &[u8], needle: &[u8]) -> bool
{
    if needle.is_empty() || haystack.len() < needle.len()
    {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Escape a filename for use in the Content-Disposition header.
/// Strips control chars and double quotes. The RFC 6266 filename*=
/// form is computed from the already-sanitized string.
pub fn sanitize_download_name(raw: &str) -> String
{
    let out: String = raw.chars()
        .filter(|c| !c.is_control() && *c != '"' && *c != '\\' && *c != '/')
        .take(200)
        .collect();
    if out.trim().is_empty()
    {
        "download".to_owned()
    }
    else
    {
        out
    }
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn accepts_basic_stl()
    {
        let (name, kind) = validate_filename("model.stl").unwrap();
        assert_eq!(name, "model.stl");
        assert_eq!(kind, FileKind::Stl);
    }

    #[test]
    fn accepts_uppercase_extension()
    {
        let (_, kind) = validate_filename("model.STL").unwrap();
        assert_eq!(kind, FileKind::Stl);
    }

    #[test]
    fn accepts_step_alias()
    {
        let (_, kind) = validate_filename("part.step").unwrap();
        assert_eq!(kind, FileKind::Step);
    }

    #[test]
    fn rejects_null_byte()
    {
        assert!(validate_filename("mo\0del.stl").is_err());
    }

    #[test]
    fn rejects_path_traversal()
    {
        assert!(validate_filename("../../etc/passwd.stl").is_err());
        assert!(validate_filename("..\\win.stl").is_err());
        assert!(validate_filename("sub/model.stl").is_err());
    }

    #[test]
    fn rejects_hidden_file()
    {
        assert!(validate_filename(".hidden.stl").is_err());
    }

    #[test]
    fn rejects_missing_extension()
    {
        assert!(validate_filename("model").is_err());
        assert!(validate_filename("model.").is_err());
    }

    #[test]
    fn rejects_unsupported_extension()
    {
        assert!(validate_filename("model.exe").is_err());
        assert!(validate_filename("model.stl.exe").is_err());
    }

    #[test]
    fn rejects_too_long()
    {
        let n = "a".repeat(260) + ".stl";
        assert!(validate_filename(&n).is_err());
    }

    #[test]
    fn stl_ascii_magic_ok()
    {
        let prefix = b"solid model\nfacet normal 0 0 0\n";
        assert!(validate_magic_bytes(FileKind::Stl, prefix, prefix.len() as u64).is_ok());
    }

    #[test]
    fn stl_binary_magic_ok()
    {
        let mut buf = vec![0u8; 84];
        buf[80..84].copy_from_slice(&2u32.to_le_bytes());
        let size = 84 + 50 * 2;
        assert!(validate_magic_bytes(FileKind::Stl, &buf, size).is_ok());
    }

    #[test]
    fn stl_binary_magic_rejects_wrong_size()
    {
        let mut buf = vec![0u8; 84];
        buf[80..84].copy_from_slice(&2u32.to_le_bytes());
        assert!(validate_magic_bytes(FileKind::Stl, &buf, 100).is_err());
    }

    #[test]
    fn stl_rejects_exe_disguised()
    {
        let prefix = b"MZ\x90\x00\x03\x00\x00\x00\x04";
        assert!(validate_magic_bytes(FileKind::Stl, prefix, 1000).is_err());
    }

    #[test]
    fn threemf_magic_ok()
    {
        let prefix = b"PK\x03\x04stuff";
        assert!(validate_magic_bytes(FileKind::ThreeMf, prefix, 1000).is_ok());
    }

    #[test]
    fn threemf_rejects_non_zip()
    {
        assert!(validate_magic_bytes(FileKind::ThreeMf, b"not zip", 1000).is_err());
    }

    #[test]
    fn step_magic_ok()
    {
        let prefix = b"HEADER;\nISO-10303-21;\n";
        assert!(validate_magic_bytes(FileKind::Step, prefix, 1000).is_ok());
    }

    #[test]
    fn step_rejects_without_marker()
    {
        assert!(validate_magic_bytes(FileKind::Step, b"random bytes", 1000).is_err());
    }

    #[test]
    fn sanitize_strips_quotes_and_controls()
    {
        let out = sanitize_download_name("he\"llo\n.stl");
        assert!(!out.contains('"'));
        assert!(!out.contains('\n'));
    }

    #[test]
    fn sanitize_falls_back_to_download_on_empty()
    {
        assert_eq!(sanitize_download_name("\0\0\0"), "download");
        assert_eq!(sanitize_download_name(""), "download");
    }

    #[test]
    fn accepts_unicode_filename()
    {
        let (name, kind) = validate_filename("modele-pièce.stl").unwrap();
        assert_eq!(name, "modele-pièce.stl");
        assert_eq!(kind, FileKind::Stl);
    }

    #[test]
    fn accepts_parentheses()
    {
        assert!(validate_filename("part (v2).3mf").is_ok());
    }

    #[test]
    fn rejects_control_chars()
    {
        assert!(validate_filename("bad\x1bname.stl").is_err());
        assert!(validate_filename("tab\there.stl").is_err());
    }
}
