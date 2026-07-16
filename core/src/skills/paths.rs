use super::{anchored::AnchoredRoot, SkillError};
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
        paths.ensure_mux_root()?;
        paths.ensure_transaction_roots()?;
        Ok(paths)
    }

    pub(crate) fn ensure_mux_root(&self) -> Result<(), SkillError> {
        create_private_root(&self.mux_dir)
    }

    pub(crate) fn ensure_transaction_roots(&self) -> Result<(), SkillError> {
        for root in [
            self.skills_dir(),
            self.staging_skills_dir(),
            self.backups_skills_dir(),
            self.journals_skills_dir(),
        ] {
            create_private_root(&root)?;
        }
        Ok(())
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

#[cfg(unix)]
fn create_private_root(path: &Path) -> Result<(), SkillError> {
    AnchoredRoot::open_or_create_private_absolute(path)?;
    Ok(())
}

#[cfg(not(unix))]
fn create_private_root(_path: &Path) -> Result<(), SkillError> {
    Err(SkillError::InvalidSource {
        message: "secure Skill transaction roots are unavailable on this platform".into(),
    })
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
