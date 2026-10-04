use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{anyhow, Error};

pub const STDIN_STDOUT: &str = "-";

/// A temporary file in the same directory as the target.  It is renamed
/// over the target on commit and deleted if dropped without commit.
struct TempFile {
    path: Option<PathBuf>,
    file: Option<File>,
}

impl TempFile {
    fn new_in(dir: &Path, target: &Path) -> io::Result<TempFile> {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let base = target
            .file_name()
            .map(|x| x.to_string_lossy())
            .unwrap_or_default();
        loop {
            let path = dir.join(format!(
                ".{}.{}-{}.tmp",
                base,
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o666);
            }
            match options.open(&path) {
                Ok(file) => {
                    return Ok(TempFile {
                        path: Some(path),
                        file: Some(file),
                    })
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(err),
            }
        }
    }

    fn file(&mut self) -> &mut File {
        self.file.as_mut().unwrap()
    }

    fn persist(mut self, target: &Path) -> io::Result<()> {
        // close the file before renaming (required on windows)
        self.file.take();
        if let Some(ref path) = self.path {
            fs::rename(path, target)?;
            self.path = None;
        }
        Ok(())
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        self.file.take();
        if let Some(ref path) = self.path {
            fs::remove_file(path).ok();
        }
    }
}

pub struct Output {
    temp: Option<(PathBuf, TempFile)>,
}

impl Output {
    pub fn new(filename: &Path) -> Result<Output, Error> {
        Ok(Output {
            temp: if filename == Path::new(STDIN_STDOUT) {
                None
            } else {
                let filename = std::env::current_dir()?.join(filename);
                let temp = TempFile::new_in(
                    filename
                        .parent()
                        .ok_or_else(|| anyhow!("cannot write to root"))?,
                    &filename,
                )?;
                Some((filename, temp))
            },
        })
    }

    pub fn commit(&mut self) -> Result<(), Error> {
        if let Some((filename, temp)) = self.temp.take() {
            temp.persist(&filename)?;
        }
        Ok(())
    }
}

impl Write for Output {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.temp {
            Some((_, ref mut out)) => out.file().write(buf),
            None => std::io::stdout().write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.temp {
            Some((_, ref mut out)) => out.file().flush(),
            None => std::io::stdout().flush(),
        }
    }
}
