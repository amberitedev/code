//! Utility functions for Theseus

/// I/O utilities
pub mod io {
    use std::path::Path;
    use tokio::fs;
    use crate::theseus::error::{TheseusError, TheseusResult};
    
    /// Create directory and all parents
    pub async fn create_dir_all(path: &Path) -> TheseusResult<()> {
        fs::create_dir_all(path).await?;
        Ok(())
    }
    
    /// Write bytes to file
    pub async fn write(path: &Path, contents: &[u8]) -> TheseusResult<()> {
        if let Some(parent) = path.parent() {
            create_dir_all(parent).await?;
        }
        fs::write(path, contents).await?;
        Ok(())
    }
    
    /// Read bytes from file
    pub async fn read(path: &Path) -> TheseusResult<Vec<u8>> {
        let contents = fs::read(path).await?;
        Ok(contents)
    }
    
    /// Check if path exists
    pub async fn exists(path: &Path) -> bool {
        path.exists()
    }
    
    /// Remove file
    pub async fn remove_file(path: &Path) -> TheseusResult<()> {
        fs::remove_file(path).await?;
        Ok(())
    }
    
    /// Remove directory recursively
    pub async fn remove_dir_all(path: &Path) -> TheseusResult<()> {
        fs::remove_dir_all(path).await?;
        Ok(())
    }
    
    /// Canonicalize path
    pub async fn canonicalize(path: &Path) -> TheseusResult<std::path::PathBuf> {
        let canonical = fs::canonicalize(path).await?;
        Ok(canonical)
    }
}

/// Fetch utilities
pub mod fetch {
    use crate::theseus::error::TheseusResult;
    
    /// Download a file with hash verification
    pub async fn download_with_sha1(
        url: &str,
        expected_sha1: Option<&str>,
    ) -> TheseusResult<Vec<u8>> {
        let response = reqwest::get(url).await?;
        let bytes = response.bytes().await?;
        let data = bytes.to_vec();
        
        if let Some(expected) = expected_sha1 {
            use sha1::Digest;
            let mut hasher = sha1::Sha1::new();
            hasher.update(&data);
            let actual = format!("{:x}", hasher.finalize());
            
            if actual != expected {
                return Err(crate::theseus::error::TheseusError::HashMismatch {
                    expected: expected.to_string(),
                    actual,
                });
            }
        }
        
        Ok(data)
    }
}
