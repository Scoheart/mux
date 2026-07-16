use super::{io_error, SkillError};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SkillsPaths {
    mux_dir: PathBuf,
    user_home: PathBuf,
}

impl SkillsPaths {
    /// Resolve the Skills roots without creating or chmod-ing anything.
    ///
    /// Read-only inventory callers must use this constructor. Lifecycle
    /// mutation paths use [`Self::from_env`] so their private roots exist.
    pub(crate) fn resolve_from_env() -> Result<Self, SkillError> {
        let mux_dir = crate::paths::mux_dir();
        if !mux_dir.is_absolute() {
            return Err(SkillError::InvalidSource {
                message: "MUX_HOME must resolve to an absolute path".into(),
            });
        }
        let user_home = dirs::home_dir()
            .filter(|path| path.is_absolute())
            .ok_or_else(|| SkillError::InvalidSource {
                message: "the user home directory is unavailable".into(),
            })?;
        Ok(Self { mux_dir, user_home })
    }

    pub fn from_env() -> Result<Self, SkillError> {
        let paths = Self::resolve_from_env()?;
        for root in [
            paths.skills_dir(),
            paths.staging_skills_dir(),
            paths.backups_skills_dir(),
            paths.journals_skills_dir(),
        ] {
            create_private_root(&root)?;
        }
        Ok(paths)
    }

    pub fn mux_dir(&self) -> &Path {
        &self.mux_dir
    }

    pub fn user_home(&self) -> &Path {
        &self.user_home
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.mux_dir.join("skills")
    }

    pub fn staging_skills_dir(&self) -> PathBuf {
        self.mux_dir.join("staging/skills")
    }

    pub fn backups_skills_dir(&self) -> PathBuf {
        self.mux_dir.join("backups/skills")
    }

    pub fn journals_skills_dir(&self) -> PathBuf {
        self.mux_dir.join("journals/skills")
    }

    /// Compatibility alias retained for the transaction fixture contract.
    /// Both names resolve to the single journal root.
    pub fn journals_dir(&self) -> PathBuf {
        self.journals_skills_dir()
    }

    pub fn skills_lock(&self) -> PathBuf {
        self.mux_dir.join("skills.lock")
    }

    pub fn central_skill(&self, name: &str) -> PathBuf {
        self.skills_dir().join(name)
    }

    pub fn expand_user(&self, value: &str) -> Option<PathBuf> {
        if value == "~" {
            return Some(self.user_home.clone());
        }
        if let Some(relative) = value.strip_prefix("~/") {
            let relative = Path::new(relative);
            if relative
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
            {
                return Some(self.user_home.join(relative));
            }
            return None;
        }
        if value.starts_with('~') {
            return None;
        }
        Some(PathBuf::from(value))
    }
}

fn create_private_root(path: &Path) -> Result<(), SkillError> {
    fs::create_dir_all(path).map_err(|error| io_error(path, error))?;
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if !metadata.file_type().is_dir() {
        return Err(SkillError::UnsafePath {
            message: "a MUX-owned Skills root is not a directory".into(),
            path: super::normalized_error_path(path),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| io_error(path, error))?;
        for directory in path.ancestors().take(3) {
            fs::File::open(directory)
                .and_then(|file| file.sync_all())
                .map_err(|error| io_error(directory, error))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testenv::TestHome;

    #[test]
    fn user_expansion_rejects_paths_that_escape_the_user_home() {
        let _th = TestHome::new("skill-user-path-expansion");
        let paths = SkillsPaths::from_env().unwrap();
        assert!(paths.expand_user("~//outside").is_none());
        assert!(paths.expand_user("~/../outside").is_none());
        assert_eq!(
            paths.expand_user("~/.agent/skills").unwrap(),
            paths.user_home().join(".agent/skills")
        );
    }
}
