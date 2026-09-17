use super::AppError;
use std::{
    fs::File,
    io::{self, Seek, Write},
    path::{Path, PathBuf},
};
use tempfile::{NamedTempFile, tempfile};

pub(crate) enum StagedSnapshot {
    Stdout(File),
    File {
        temporary: NamedTempFile,
        destination: PathBuf,
    },
}

impl StagedSnapshot {
    pub(crate) fn new(destination: Option<PathBuf>) -> Result<Self, AppError> {
        let Some(destination) = destination else {
            return Ok(Self::Stdout(tempfile()?));
        };
        let directory = output_directory(&destination);
        if !directory.is_dir() {
            return Err(AppError::OutputDirectoryMissing(directory));
        }
        let temporary = NamedTempFile::new_in(&directory)
            .map_err(|source| AppError::CreateOutputStagingFile { directory, source })?;
        Ok(Self::File {
            temporary,
            destination,
        })
    }

    pub(crate) fn file_mut(&mut self) -> &mut File {
        match self {
            Self::Stdout(file) => file,
            Self::File { temporary, .. } => temporary.as_file_mut(),
        }
    }

    pub(crate) fn commit(self) -> Result<Option<PathBuf>, AppError> {
        match self {
            Self::Stdout(mut file) => {
                file.rewind()?;
                let stdout = io::stdout();
                let mut stdout = stdout.lock();
                io::copy(&mut file, &mut stdout)?;
                stdout.flush()?;
                Ok(None)
            }
            Self::File {
                temporary,
                destination,
            } => {
                temporary.as_file().sync_all()?;
                temporary
                    .persist(&destination)
                    .map_err(|error| AppError::FinalizeOutput {
                        path: destination.clone(),
                        source: error.error,
                    })?;
                Ok(Some(destination))
            }
        }
    }
}

fn output_directory(destination: &Path) -> PathBuf {
    destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::StagedSnapshot;
    use std::{fs, io::Write};
    use tempfile::tempdir;

    #[test]
    fn dropping_staged_file_preserves_existing_destination() {
        let directory = tempdir().unwrap();
        let destination = directory.path().join("snapshot.json");
        fs::write(&destination, b"existing").unwrap();
        let mut staged = StagedSnapshot::new(Some(destination.clone())).unwrap();
        staged.file_mut().write_all(b"incomplete").unwrap();
        drop(staged);
        assert_eq!(fs::read(destination).unwrap(), b"existing");
    }

    #[test]
    fn committing_staged_file_replaces_destination() {
        let directory = tempdir().unwrap();
        let destination = directory.path().join("snapshot.json");
        fs::write(&destination, b"existing").unwrap();
        let mut staged = StagedSnapshot::new(Some(destination.clone())).unwrap();
        staged.file_mut().write_all(b"complete").unwrap();
        let committed = staged.commit().unwrap();
        assert_eq!(committed.as_deref(), Some(destination.as_path()));
        assert_eq!(fs::read(destination).unwrap(), b"complete");
    }
}
