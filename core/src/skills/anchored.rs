use super::{io_error, normalized_error_path, SkillError};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AnchoredFileKind {
    Regular,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AnchoredIdentity {
    pub kind: AnchoredFileKind,
    pub device: u64,
    pub inode: u64,
    pub links: u64,
    pub size: u64,
    pub mode: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConsumedFile {
    pub size: u64,
    pub sha256: String,
}

pub(super) fn verify_anchored_identity(
    expected: &AnchoredIdentity,
    actual: &AnchoredIdentity,
    path: &Path,
) -> Result<(), SkillError> {
    if actual.kind != AnchoredFileKind::Regular {
        return Err(SkillError::UnsafePath {
            message: "a validated Skill file was replaced with a non-regular entry".into(),
            path: normalized_error_path(path),
        });
    }
    if actual.links != 1 {
        return Err(SkillError::UnsafePath {
            message: "hard-linked files are not allowed in Skill trees".into(),
            path: normalized_error_path(path),
        });
    }
    if expected.kind != actual.kind
        || expected.device != actual.device
        || expected.inode != actual.inode
        || expected.links != actual.links
        || expected.size != actual.size
        || expected.mode != actual.mode
    {
        return Err(SkillError::Conflict {
            message: "a Skill file changed identity after inspection".into(),
            path: normalized_error_path(path),
        });
    }
    Ok(())
}

pub(super) fn verify_anchored_digest(
    expected: &str,
    actual: &str,
    path: &Path,
) -> Result<(), SkillError> {
    if expected != actual {
        return Err(SkillError::Conflict {
            message: "a Skill file changed content after inspection".into(),
            path: normalized_error_path(path),
        });
    }
    Ok(())
}

pub(super) fn consume_bounded_and_hash<R: Read, W: Write>(
    mut reader: R,
    writer: &mut W,
    expected_size: u64,
    maximum: u64,
    path: &Path,
    limit: &'static str,
) -> Result<ConsumedFile, SkillError> {
    if expected_size > maximum {
        return Err(SkillError::LimitExceeded {
            limit: limit.into(),
            actual: expected_size,
            allowed: maximum,
        });
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut size = 0_u64;
    loop {
        let remaining = maximum.saturating_add(1).saturating_sub(size);
        if remaining == 0 {
            break;
        }
        let requested = buffer.len().min(remaining as usize);
        let read = reader
            .read(&mut buffer[..requested])
            .map_err(|error| io_error(path, error))?;
        if read == 0 {
            break;
        }
        size = size.saturating_add(read as u64);
        if size > maximum {
            return Err(SkillError::LimitExceeded {
                limit: limit.into(),
                actual: size,
                allowed: maximum,
            });
        }
        hasher.update(&buffer[..read]);
        writer
            .write_all(&buffer[..read])
            .map_err(|error| io_error(path, error))?;
    }
    if size != expected_size {
        return Err(SkillError::Conflict {
            message: "a Skill file changed size while being read".into(),
            path: normalized_error_path(path),
        });
    }
    Ok(ConsumedFile {
        size,
        sha256: hex::encode(hasher.finalize()),
    })
}

#[cfg(unix)]
mod platform {
    use super::*;
    use rustix::fs::{
        fchmod, fstat, mkdirat, openat, readlinkat, renameat_with, statat, symlinkat, unlinkat,
        AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, Stat, CWD,
    };
    use rustix::io::Errno;
    use std::collections::VecDeque;
    use std::ffi::{CStr, CString, OsStr};
    use std::fs::{self, File};
    use std::path::{Component, Path, PathBuf};

    pub(crate) struct AnchoredRoot {
        directory: File,
        canonical_path: PathBuf,
    }

    impl AnchoredRoot {
        pub(crate) fn open_or_create_private_absolute(path: &Path) -> Result<Self, SkillError> {
            Self::open_or_create_absolute_inner(path, true)
        }

        pub(crate) fn open_or_create_absolute(path: &Path) -> Result<Self, SkillError> {
            Self::open_or_create_absolute_inner(path, false)
        }

        fn open_or_create_absolute_inner(
            path: &Path,
            tighten_permissions: bool,
        ) -> Result<Self, SkillError> {
            let components = path
                .components()
                .map(|component| match component {
                    Component::RootDir => Ok(None),
                    Component::Normal(name) => Ok(Some(name)),
                    _ => Err(unsafe_path(path)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !matches!(path.components().next(), Some(Component::RootDir)) {
                return Err(unsafe_path(path));
            }

            let mut directory = File::from(
                openat(CWD, Path::new("/"), directory_flags(), Mode::empty())
                    .map_err(|error| io_error(Path::new("/"), error.into()))?,
            );
            let names = components.into_iter().flatten().collect::<Vec<_>>();
            for (index, name) in names.iter().enumerate() {
                let child = match openat(&directory, *name, directory_flags(), Mode::empty()) {
                    Ok(child) => child,
                    Err(Errno::NOENT) => {
                        match mkdirat(&directory, *name, Mode::from(0o700)) {
                            Ok(()) | Err(Errno::EXIST) => {}
                            Err(error) => return Err(io_error(path, error.into())),
                        }
                        directory
                            .sync_all()
                            .map_err(|error| io_error(path, error))?;
                        openat(&directory, *name, directory_flags(), Mode::empty())
                            .map_err(|_| unsafe_path(path))?
                    }
                    Err(_) => return Err(unsafe_path(path)),
                };
                directory = File::from(child);
                if index + 1 == names.len() {
                    if tighten_permissions {
                        fchmod(&directory, Mode::from(0o700))
                            .map_err(|error| io_error(path, error.into()))?;
                    }
                    directory
                        .sync_all()
                        .map_err(|error| io_error(path, error))?;
                }
            }
            let expected = identity_from_stat(
                &fstat(&directory).map_err(|error| io_error(path, error.into()))?,
            );
            Self::from_open_directory(directory, path.to_path_buf(), &expected)
        }

        pub(crate) fn open(path: &Path) -> Result<Self, SkillError> {
            let expected = Self::inspect_directory(path)?;
            Self::open_expected(path, &expected)
        }

        pub(crate) fn inspect_directory(path: &Path) -> Result<AnchoredIdentity, SkillError> {
            let expected = identity_from_stat(
                &statat(CWD, path, AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(|error| io_error(path, error.into()))?,
            );
            if expected.kind != AnchoredFileKind::Directory {
                return Err(SkillError::UnsafePath {
                    message: "Skill root must be a directory, not a symlink or file".into(),
                    path: normalized_error_path(path),
                });
            }
            Ok(expected)
        }

        pub(crate) fn open_expected(
            path: &Path,
            expected: &AnchoredIdentity,
        ) -> Result<Self, SkillError> {
            if expected.kind != AnchoredFileKind::Directory {
                return Err(unsafe_type(path));
            }
            let directory = File::from(
                openat(CWD, path, directory_flags(), Mode::empty())
                    .map_err(|error| io_error(path, error.into()))?,
            );
            let opened = identity_from_stat(
                &fstat(&directory).map_err(|error| io_error(path, error.into()))?,
            );
            verify_directory_identity(expected, &opened, path)?;

            let canonical_path = fs::canonicalize(path).map_err(|error| io_error(path, error))?;
            let canonical = identity_from_stat(
                &statat(CWD, &canonical_path, AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(|error| io_error(path, error.into()))?,
            );
            verify_directory_identity(&opened, &canonical, path)?;
            Ok(Self {
                directory,
                canonical_path,
            })
        }

        pub(crate) fn from_open_directory(
            directory: File,
            canonical_path: PathBuf,
            expected: &AnchoredIdentity,
        ) -> Result<Self, SkillError> {
            let actual = identity_from_stat(
                &fstat(&directory).map_err(|error| io_error(&canonical_path, error.into()))?,
            );
            verify_directory_identity(expected, &actual, &canonical_path)?;
            Ok(Self {
                directory,
                canonical_path,
            })
        }

        pub(crate) fn try_clone(&self) -> Result<Self, SkillError> {
            let directory = self
                .directory
                .try_clone()
                .map_err(|error| io_error(&self.canonical_path, error))?;
            let expected = identity_from_stat(
                &fstat(&self.directory)
                    .map_err(|error| io_error(&self.canonical_path, error.into()))?,
            );
            Self::from_open_directory(directory, self.canonical_path.clone(), &expected)
        }

        pub(crate) fn canonical_path(&self) -> &Path {
            &self.canonical_path
        }

        pub(crate) fn root_directory(&self) -> Result<File, SkillError> {
            self.directory
                .try_clone()
                .map_err(|error| io_error(&self.canonical_path, error))
        }

        pub(crate) fn identity(&self) -> Result<AnchoredIdentity, SkillError> {
            fstat(&self.directory)
                .map(|stat| identity_from_stat(&stat))
                .map_err(|error| io_error(&self.canonical_path, error.into()))
        }

        pub(crate) fn path_refers_to_root(&self, path: &Path) -> Result<bool, SkillError> {
            let expected = self.identity()?;
            let actual = match statat(CWD, path, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => identity_from_stat(&stat),
                Err(Errno::NOENT) => return Ok(false),
                Err(error) => return Err(io_error(path, error.into())),
            };
            Ok(expected.kind == AnchoredFileKind::Directory
                && actual.kind == AnchoredFileKind::Directory
                && expected.device == actual.device
                && expected.inode == actual.inode)
        }

        pub(crate) fn create_symlink_entry(
            &self,
            target: &Path,
            name: &OsStr,
            path: &Path,
        ) -> Result<(), SkillError> {
            symlinkat(target, &self.directory, name)
                .map_err(|error| io_error(path, error.into()))?;
            self.directory
                .sync_all()
                .map_err(|error| io_error(path, error))
        }

        pub(crate) fn create_file_entry(
            &self,
            name: &OsStr,
            mode: u16,
            path: &Path,
        ) -> Result<File, SkillError> {
            openat(
                &self.directory,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::from(mode),
            )
            .map(File::from)
            .map_err(|error| io_error(path, error.into()))
        }

        pub(crate) fn stat_root_entry(
            &self,
            name: &OsStr,
            path: &Path,
        ) -> Result<Option<AnchoredIdentity>, SkillError> {
            match statat(&self.directory, name, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => Ok(Some(identity_from_stat(&stat))),
                Err(Errno::NOENT) => Ok(None),
                Err(error) => Err(io_error(path, error.into())),
            }
        }

        pub(crate) fn read_link_root_entry(
            &self,
            name: &OsStr,
            expected: &AnchoredIdentity,
            path: &Path,
        ) -> Result<Vec<u8>, SkillError> {
            let name = CString::new(name.as_encoded_bytes()).map_err(|_| unsafe_path(path))?;
            let directory = self.root_directory()?;
            self.read_link_entry(&directory, &name, expected, path)
        }

        pub(crate) fn unlink_root_entry(
            &self,
            name: &OsStr,
            is_directory: bool,
            path: &Path,
        ) -> Result<(), SkillError> {
            let name = CString::new(name.as_encoded_bytes()).map_err(|_| unsafe_path(path))?;
            let directory = self.root_directory()?;
            self.unlink_entry(&directory, &name, is_directory, path)
        }

        pub(crate) fn rename_entry_noreplace(
            &self,
            from: &OsStr,
            to: &OsStr,
            path: &Path,
        ) -> Result<(), SkillError> {
            renameat_with(
                &self.directory,
                from,
                &self.directory,
                to,
                RenameFlags::NOREPLACE,
            )
            .map_err(|error| io_error(path, error.into()))?;
            self.directory
                .sync_all()
                .map_err(|error| io_error(path, error))
        }

        pub(crate) fn rename_entry_noreplace_to(
            &self,
            from: &OsStr,
            destination: &Self,
            to: &OsStr,
            path: &Path,
        ) -> Result<(), SkillError> {
            renameat_with(
                &self.directory,
                from,
                &destination.directory,
                to,
                RenameFlags::NOREPLACE,
            )
            .map_err(|error| io_error(path, error.into()))?;
            self.directory
                .sync_all()
                .map_err(|error| io_error(path, error))?;
            destination
                .directory
                .sync_all()
                .map_err(|error| io_error(path, error))
        }

        pub(crate) fn exchange_entries(
            &self,
            left: &OsStr,
            right: &OsStr,
            path: &Path,
        ) -> Result<(), SkillError> {
            renameat_with(
                &self.directory,
                left,
                &self.directory,
                right,
                RenameFlags::EXCHANGE,
            )
            .map_err(|error| io_error(path, error.into()))?;
            self.directory
                .sync_all()
                .map_err(|error| io_error(path, error))
        }

        pub(crate) fn unlink_entry(
            &self,
            directory: &File,
            name: &CStr,
            is_directory: bool,
            path: &Path,
        ) -> Result<(), SkillError> {
            let flags = if is_directory {
                AtFlags::REMOVEDIR
            } else {
                AtFlags::empty()
            };
            unlinkat(directory, name, flags).map_err(|error| io_error(path, error.into()))?;
            directory.sync_all().map_err(|error| io_error(path, error))
        }

        pub(crate) fn read_directory(
            &self,
            directory: &File,
            path: &Path,
        ) -> Result<Vec<CString>, SkillError> {
            self.read_directory_budgeted(directory, path, 0, u64::MAX, "entries")
        }

        pub(crate) fn read_directory_budgeted(
            &self,
            directory: &File,
            path: &Path,
            starting_count: u64,
            allowed: u64,
            limit: &'static str,
        ) -> Result<Vec<CString>, SkillError> {
            let mut names = Vec::new();
            let entries =
                Dir::read_from(directory).map_err(|error| io_error(path, error.into()))?;
            for entry in entries {
                let entry = entry.map_err(|error| io_error(path, error.into()))?;
                let name = entry.file_name();
                if name.to_bytes() != b"." && name.to_bytes() != b".." {
                    let actual = starting_count.saturating_add(names.len() as u64 + 1);
                    if actual > allowed {
                        return Err(SkillError::LimitExceeded {
                            limit: limit.into(),
                            actual,
                            allowed,
                        });
                    }
                    names.push(name.to_owned());
                }
            }
            names.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
            Ok(names)
        }

        pub(crate) fn stat_entry(
            &self,
            directory: &File,
            name: &CStr,
            path: &Path,
        ) -> Result<AnchoredIdentity, SkillError> {
            statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)
                .map(|stat| identity_from_stat(&stat))
                .map_err(|error| io_error(path, error.into()))
        }

        pub(crate) fn open_directory_entry(
            &self,
            directory: &File,
            name: &CStr,
            expected: &AnchoredIdentity,
            path: &Path,
        ) -> Result<File, SkillError> {
            if expected.kind != AnchoredFileKind::Directory {
                return Err(unsafe_type(path));
            }
            let opened = File::from(
                openat(directory, name, directory_flags(), Mode::empty())
                    .map_err(|error| io_error(path, error.into()))?,
            );
            let actual =
                identity_from_stat(&fstat(&opened).map_err(|error| io_error(path, error.into()))?);
            verify_directory_identity(expected, &actual, path)?;
            Ok(opened)
        }

        pub(crate) fn open_regular_entry(
            &self,
            directory: &File,
            name: &CStr,
            expected: &AnchoredIdentity,
            path: &Path,
        ) -> Result<File, SkillError> {
            if expected.kind != AnchoredFileKind::Regular {
                return Err(unsafe_type(path));
            }
            let opened = File::from(
                openat(directory, name, file_flags(), Mode::empty())
                    .map_err(|error| io_error(path, error.into()))?,
            );
            let actual =
                identity_from_stat(&fstat(&opened).map_err(|error| io_error(path, error.into()))?);
            verify_anchored_identity(expected, &actual, path)?;
            Ok(opened)
        }

        pub(crate) fn verify_regular_file(
            &self,
            file: &File,
            expected: &AnchoredIdentity,
            path: &Path,
        ) -> Result<(), SkillError> {
            let actual =
                identity_from_stat(&fstat(file).map_err(|error| io_error(path, error.into()))?);
            verify_anchored_identity(expected, &actual, path)
        }

        pub(crate) fn open_regular_relative(
            &self,
            relative: &str,
            expected: &AnchoredIdentity,
            path: &Path,
        ) -> Result<File, SkillError> {
            let (directory, name) = self.open_parent(relative, path)?;
            self.open_regular_entry(&directory, &name, expected, path)
        }

        pub(crate) fn read_link_entry(
            &self,
            directory: &File,
            name: &CStr,
            expected: &AnchoredIdentity,
            path: &Path,
        ) -> Result<Vec<u8>, SkillError> {
            validate_supported_links(expected, path)?;
            if expected.kind != AnchoredFileKind::Symlink {
                return Err(unsafe_type(path));
            }
            let target = readlinkat(directory, name, Vec::new())
                .map_err(|error| io_error(path, error.into()))?
                .into_bytes();
            let actual = self.stat_entry(directory, name, path)?;
            verify_symlink_identity(expected, &actual, path)?;
            if target.len() as u64 != expected.size {
                return Err(SkillError::Conflict {
                    message: "a Skill symlink changed size while being read".into(),
                    path: normalized_error_path(path),
                });
            }
            Ok(target)
        }

        pub(crate) fn validate_symlink_target(
            &self,
            link_path: &str,
            target: &str,
            path: &Path,
        ) -> Result<(), SkillError> {
            let parent: Vec<String> = link_path
                .split('/')
                .rev()
                .skip(1)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(str::to_string)
                .collect();
            let mut pending = normalize_target(&parent, target, VecDeque::new(), path)?;
            let mut symlink_hops = 0_u8;

            'restart: loop {
                if pending.is_empty() {
                    return Ok(());
                }
                let mut directory = self.root_directory()?;
                let mut resolved_parent = Vec::new();
                while let Some(component) = pending.pop_front() {
                    let name = CString::new(component.as_bytes()).map_err(|_| unsafe_path(path))?;
                    let identity = self.stat_entry(&directory, &name, path)?;
                    if identity.kind == AnchoredFileKind::Symlink {
                        symlink_hops = symlink_hops.saturating_add(1);
                        if symlink_hops > 40 {
                            return Err(SkillError::UnsafePath {
                                message: "Skill symlink chain exceeds the safe resolution limit"
                                    .into(),
                                path: normalized_error_path(path),
                            });
                        }
                        let target = self.read_link_entry(&directory, &name, &identity, path)?;
                        let target = std::str::from_utf8(&target).map_err(|_| unsafe_path(path))?;
                        pending = normalize_target(&resolved_parent, target, pending, path)?;
                        continue 'restart;
                    }
                    if pending.is_empty() {
                        return match identity.kind {
                            AnchoredFileKind::Regular | AnchoredFileKind::Directory => Ok(()),
                            _ => Err(unsafe_type(path)),
                        };
                    }
                    if identity.kind != AnchoredFileKind::Directory {
                        return Err(unsafe_type(path));
                    }
                    directory = self.open_directory_entry(&directory, &name, &identity, path)?;
                    resolved_parent.push(component);
                }
            }
        }

        fn open_parent(&self, relative: &str, path: &Path) -> Result<(File, CString), SkillError> {
            let mut components = relative.split('/').peekable();
            let mut directory = self.root_directory()?;
            while let Some(component) = components.next() {
                let name = CString::new(component.as_bytes()).map_err(|_| unsafe_path(path))?;
                if components.peek().is_none() {
                    return Ok((directory, name));
                }
                let identity = self.stat_entry(&directory, &name, path)?;
                directory = self.open_directory_entry(&directory, &name, &identity, path)?;
            }
            Err(unsafe_path(path))
        }
    }

    pub(crate) fn validate_supported_links(
        identity: &AnchoredIdentity,
        path: &Path,
    ) -> Result<(), SkillError> {
        if matches!(
            identity.kind,
            AnchoredFileKind::Regular | AnchoredFileKind::Symlink
        ) && identity.links != 1
        {
            return Err(SkillError::UnsafePath {
                message: "hard-linked files and symlinks are not allowed in Skill trees".into(),
                path: normalized_error_path(path),
            });
        }
        Ok(())
    }

    fn verify_directory_identity(
        expected: &AnchoredIdentity,
        actual: &AnchoredIdentity,
        path: &Path,
    ) -> Result<(), SkillError> {
        if expected.kind != AnchoredFileKind::Directory
            || actual.kind != AnchoredFileKind::Directory
        {
            return Err(unsafe_type(path));
        }
        if expected.device != actual.device
            || expected.inode != actual.inode
            || expected.mode != actual.mode
        {
            return Err(SkillError::Conflict {
                message: "a Skill directory changed identity after inspection".into(),
                path: normalized_error_path(path),
            });
        }
        Ok(())
    }

    fn verify_symlink_identity(
        expected: &AnchoredIdentity,
        actual: &AnchoredIdentity,
        path: &Path,
    ) -> Result<(), SkillError> {
        validate_supported_links(actual, path)?;
        if expected.kind != AnchoredFileKind::Symlink
            || actual.kind != AnchoredFileKind::Symlink
            || expected.device != actual.device
            || expected.inode != actual.inode
            || expected.links != actual.links
            || expected.size != actual.size
            || expected.mode != actual.mode
        {
            return Err(SkillError::Conflict {
                message: "a Skill symlink changed identity after inspection".into(),
                path: normalized_error_path(path),
            });
        }
        Ok(())
    }

    fn identity_from_stat(stat: &Stat) -> AnchoredIdentity {
        AnchoredIdentity {
            kind: match FileType::from_raw_mode(stat.st_mode as _) {
                FileType::RegularFile => AnchoredFileKind::Regular,
                FileType::Directory => AnchoredFileKind::Directory,
                FileType::Symlink => AnchoredFileKind::Symlink,
                _ => AnchoredFileKind::Other,
            },
            device: stat.st_dev as u64,
            inode: stat.st_ino as u64,
            links: stat.st_nlink as u64,
            size: u64::try_from(stat.st_size).unwrap_or(u64::MAX),
            mode: stat.st_mode as u32,
        }
    }

    fn normalize_target(
        base: &[String],
        target: &str,
        suffix: VecDeque<String>,
        path: &Path,
    ) -> Result<VecDeque<String>, SkillError> {
        let mut normalized: VecDeque<String> = base.iter().cloned().collect();
        for component in Path::new(target).components() {
            match component {
                Component::Normal(value) => normalized
                    .push_back(value.to_str().ok_or_else(|| unsafe_path(path))?.to_string()),
                Component::CurDir => {}
                Component::ParentDir => {
                    if normalized.pop_back().is_none() {
                        return Err(SkillError::UnsafePath {
                            message: "Skill symlink target escapes the Skill root".into(),
                            path: normalized_error_path(path),
                        });
                    }
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(SkillError::UnsafePath {
                        message: "Skill symlinks must use relative targets".into(),
                        path: normalized_error_path(path),
                    });
                }
            }
        }
        normalized.extend(suffix);
        Ok(normalized)
    }

    fn directory_flags() -> OFlags {
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
    }

    fn file_flags() -> OFlags {
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC
    }

    fn unsafe_type(path: &Path) -> SkillError {
        SkillError::UnsafePath {
            message: "unsupported file types are not allowed in Skill trees".into(),
            path: normalized_error_path(path),
        }
    }

    fn unsafe_path(path: &Path) -> SkillError {
        SkillError::UnsafePath {
            message: "Skill entry path is unsafe".into(),
            path: normalized_error_path(path),
        }
    }
}

#[cfg(unix)]
pub(super) use platform::{validate_supported_links, AnchoredRoot};

#[cfg(not(unix))]
pub(super) struct AnchoredRoot {
    canonical_path: std::path::PathBuf,
}

#[cfg(not(unix))]
impl AnchoredRoot {
    pub(super) fn open_or_create_private_absolute(_path: &Path) -> Result<Self, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn open_or_create_absolute(_path: &Path) -> Result<Self, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn open(_path: &Path) -> Result<Self, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    pub(super) fn inspect_directory(_path: &Path) -> Result<AnchoredIdentity, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn open_expected(
        _path: &Path,
        _expected: &AnchoredIdentity,
    ) -> Result<Self, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn from_open_directory(
        _directory: std::fs::File,
        _canonical_path: std::path::PathBuf,
        _expected: &AnchoredIdentity,
    ) -> Result<Self, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn try_clone(&self) -> Result<Self, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn root_directory(&self) -> Result<std::fs::File, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn identity(&self) -> Result<AnchoredIdentity, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn path_refers_to_root(&self, _path: &Path) -> Result<bool, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn create_symlink_entry(
        &self,
        _target: &Path,
        _name: &std::ffi::OsStr,
        _path: &Path,
    ) -> Result<(), SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn create_file_entry(
        &self,
        _name: &std::ffi::OsStr,
        _mode: u16,
        _path: &Path,
    ) -> Result<std::fs::File, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn stat_root_entry(
        &self,
        _name: &std::ffi::OsStr,
        _path: &Path,
    ) -> Result<Option<AnchoredIdentity>, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn read_link_root_entry(
        &self,
        _name: &std::ffi::OsStr,
        _expected: &AnchoredIdentity,
        _path: &Path,
    ) -> Result<Vec<u8>, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn unlink_root_entry(
        &self,
        _name: &std::ffi::OsStr,
        _is_directory: bool,
        _path: &Path,
    ) -> Result<(), SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn rename_entry_noreplace(
        &self,
        _from: &std::ffi::OsStr,
        _to: &std::ffi::OsStr,
        _path: &Path,
    ) -> Result<(), SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn exchange_entries(
        &self,
        _left: &std::ffi::OsStr,
        _right: &std::ffi::OsStr,
        _path: &Path,
    ) -> Result<(), SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn rename_entry_noreplace_to(
        &self,
        _from: &std::ffi::OsStr,
        _destination: &Self,
        _to: &std::ffi::OsStr,
        _path: &Path,
    ) -> Result<(), SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn unlink_entry(
        &self,
        _directory: &std::fs::File,
        _name: &std::ffi::CStr,
        _is_directory: bool,
        _path: &Path,
    ) -> Result<(), SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn read_directory_budgeted(
        &self,
        _directory: &std::fs::File,
        _path: &Path,
        _starting_count: u64,
        _allowed: u64,
        _limit: &'static str,
    ) -> Result<Vec<std::ffi::CString>, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn stat_entry(
        &self,
        _directory: &std::fs::File,
        _name: &std::ffi::CStr,
        _path: &Path,
    ) -> Result<AnchoredIdentity, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn open_directory_entry(
        &self,
        _directory: &std::fs::File,
        _name: &std::ffi::CStr,
        _expected: &AnchoredIdentity,
        _path: &Path,
    ) -> Result<std::fs::File, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn open_regular_entry(
        &self,
        _directory: &std::fs::File,
        _name: &std::ffi::CStr,
        _expected: &AnchoredIdentity,
        _path: &Path,
    ) -> Result<std::fs::File, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn read_link_entry(
        &self,
        _directory: &std::fs::File,
        _name: &std::ffi::CStr,
        _expected: &AnchoredIdentity,
        _path: &Path,
    ) -> Result<Vec<u8>, SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn verify_regular_file(
        &self,
        _file: &std::fs::File,
        _expected: &AnchoredIdentity,
        _path: &Path,
    ) -> Result<(), SkillError> {
        Err(unsupported_platform())
    }

    pub(super) fn open_regular_relative(
        &self,
        _relative: &str,
        _expected: &AnchoredIdentity,
        _path: &Path,
    ) -> Result<std::fs::File, SkillError> {
        Err(unsupported_platform())
    }
}

#[cfg(not(unix))]
pub(super) fn unsupported_platform() -> SkillError {
    SkillError::InvalidSource {
        message: "secure Skill filesystem access is unavailable on this platform".into(),
    }
}
