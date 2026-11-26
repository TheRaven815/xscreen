use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::{HSTRING, w};

use crate::i18n::{Lang, Strings};

use super::{EXE, MAX_EXE, PAYLOAD, STAGE_PREFIX, STAGE_SUFFIX_LEN, Staged};
use super::{is_sha256, is_windows_x64_exe};

const APPLY: &str = "--xscreen-apply-update";
const CLEANUP: &str = "--xscreen-cleanup-update";
const HELPER: &str = "xscreen-update-helper.exe";
const BACKUP: &str = "previous.exe";
const REPARSE_POINT: u32 = 0x400;

pub fn install_after_exit(staged: Staged, lang: Lang) {
    let helper = staged.dir.path().join(HELPER);
    let result = (|| {
        let target = staged.target.file_name().ok_or_else(invalid_stage)?;
        let mut copy = File::create_new(&helper)?;
        io::copy(&mut File::open(&staged.target)?, &mut copy)?;
        copy.sync_all()?;
        drop(copy);
        Command::new(&helper)
            .arg(APPLY)
            .arg(target)
            .arg(&staged.sha256)
            .arg(lang.code())
            .spawn()?;
        Ok::<_, io::Error>(())
    })();
    match result {
        Ok(()) => {
            // The restarted app owns cleanup from here.
            let _ = staged.dir.keep();
        }
        Err(error) => {
            tracing::error!(%error, "update helper could not start");
            show_failure(lang, None, None);
            let _ = Command::new(&staged.target).spawn();
        }
    }
}

pub fn helper_mode() -> Option<bool> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(OsStr::new(APPLY)) {
        return None;
    }
    let Ok([name, hash, lang]): Result<[OsString; 3], _> = args.collect::<Vec<_>>().try_into()
    else {
        return Some(false);
    };
    let lang = Lang::from_code(&lang.to_string_lossy());
    let result = helper_paths(&name).and_then(|(stage, target)| {
        let hash = hash
            .to_str()
            .filter(|s| is_sha256(s))
            .ok_or_else(invalid_stage)?;
        match replace_and_restart(&stage, &target, hash) {
            Ok(()) => Ok(()),
            Err(failure) => {
                show_failure(lang, Some(&failure.error), failure.backup.as_deref());
                if failure.backup.is_none() {
                    let _ = restart(&stage, &target);
                }
                Err(failure.error)
            }
        }
    });
    Some(result.is_ok())
}

fn helper_paths(name: &OsStr) -> io::Result<(PathBuf, PathBuf)> {
    let helper = std::env::current_exe()?.canonicalize()?;
    if helper.file_name() != Some(OsStr::new(HELPER)) || !single_name(name) {
        return Err(invalid_stage());
    }
    let stage = helper.parent().ok_or_else(invalid_stage)?;
    if !is_stage_dir(stage) {
        return Err(invalid_stage());
    }
    let target = stage.parent().ok_or_else(invalid_stage)?.join(name);
    if !regular_file(&target)? {
        return Err(invalid_stage());
    }
    Ok((stage.to_owned(), target))
}

#[derive(Debug)]
struct InstallFailure {
    error: io::Error,
    backup: Option<PathBuf>,
}

impl From<io::Error> for InstallFailure {
    fn from(error: io::Error) -> Self {
        Self {
            error,
            backup: None,
        }
    }
}

fn replace_and_restart(stage: &Path, target: &Path, hash: &str) -> Result<(), InstallFailure> {
    let payload = stage.join(PAYLOAD);
    if !regular_file(&payload)? {
        return Err(invalid_stage().into());
    }
    // Recheck on disk after handing off to a separate process.
    let mut file = File::open(&payload)?;
    let len = file.metadata()?.len();
    if len > MAX_EXE || !is_windows_x64_exe(&mut file, len)? {
        return Err(invalid_stage().into());
    }
    file.rewind()?;
    let mut digest = Sha256::new();
    io::copy(&mut (&mut file).take(MAX_EXE + 1), &mut digest)?;
    if format!("{:x}", digest.finalize()) != hash {
        return Err(invalid_stage().into());
    }
    drop(file);

    replace_executable(stage, target, || restart(stage, target))
}

fn replace_executable(
    stage: &Path,
    target: &Path,
    start_new: impl FnOnce() -> io::Result<()>,
) -> Result<(), InstallFailure> {
    let backup = stage.join(BACKUP);
    // Keep the original name intact until both the new file and backup are complete.
    let mut source = File::open(target)?;
    let mut saved = File::create_new(&backup)?;
    io::copy(&mut source, &mut saved)?;
    saved.flush()?;
    saved.sync_all()?;
    drop((source, saved));
    retry_rename(&stage.join(PAYLOAD), target)?;

    if let Err(error) = start_new() {
        let backup = retry_rename(&backup, target).err().map(|_| backup);
        return Err(InstallFailure { error, backup });
    }
    Ok(())
}

fn retry_rename(source: &Path, target: &Path) -> io::Result<()> {
    // The old PE can remain locked briefly while Windows tears down the old process.
    let start = Instant::now();
    loop {
        match fs::rename(source, target) {
            Err(e)
                if matches!(e.raw_os_error(), Some(5 | 32 | 33))
                    && start.elapsed() < Duration::from_secs(4) =>
            {
                std::thread::sleep(Duration::from_millis(100));
            }
            result => return result,
        }
    }
}

fn restart(stage: &Path, target: &Path) -> io::Result<()> {
    Command::new(target)
        .arg(CLEANUP)
        .arg(stage.file_name().ok_or_else(invalid_stage)?)
        .spawn()?;
    Ok(())
}

pub fn cleanup_after_start() {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(OsStr::new(CLEANUP)) {
        return;
    }
    let Some(name) = args.next().filter(|name| stage_name(name)) else {
        return;
    };
    let Ok(exe) = std::env::current_exe().and_then(fs::canonicalize) else {
        return;
    };
    let Some(folder) = exe.parent() else { return };
    let stage = folder.join(name);
    if is_stage_dir(&stage) {
        let _ = std::thread::Builder::new()
            .name("xscreen-update-cleanup".into())
            .spawn(move || {
                for _ in 0..50 {
                    let result = fs::remove_file(stage.join(HELPER));
                    if result.is_ok() || result.is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
                    {
                        // Never recursively delete a directory passed via the command line.
                        let _ = fs::remove_file(stage.join(BACKUP));
                        let _ = fs::remove_file(stage.join(PAYLOAD));
                        let _ = fs::remove_dir(stage);
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
    }
}

fn single_name(name: &OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;
    let mut components = Path::new(name).components();
    matches!(components.next(), Some(Component::Normal(part)) if part == name)
        && components.next().is_none()
        && !name.encode_wide().any(|ch| ch == u16::from(b':'))
}

fn stage_name(name: &OsStr) -> bool {
    name.to_str()
        .and_then(|s| s.strip_prefix(STAGE_PREFIX))
        .is_some_and(|s| {
            s.len() == STAGE_SUFFIX_LEN && s.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

fn is_stage_dir(path: &Path) -> bool {
    path.file_name().is_some_and(stage_name)
        && fs::symlink_metadata(path)
            .is_ok_and(|m| m.is_dir() && m.file_attributes() & REPARSE_POINT == 0)
}

fn regular_file(path: &Path) -> io::Result<bool> {
    let meta = fs::symlink_metadata(path)?;
    Ok(meta.is_file() && meta.file_attributes() & REPARSE_POINT == 0)
}

fn invalid_stage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid xscreen update staging data",
    )
}

fn show_failure(lang: Lang, error: Option<&io::Error>, backup: Option<&Path>) {
    let s = Strings::new(lang);
    let mut message = s.update_install_failed(EXE);
    if let Some(backup) = backup {
        message.push_str(&s.update_backup_path(&backup.display().to_string()));
    }
    if let Some(code) = error.and_then(io::Error::raw_os_error) {
        message.push_str(&format!("\n\nWindows: {code}"));
    }
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            w!("xscreen"),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, tempfile::TempDir, PathBuf) {
        let parent = tempfile::tempdir().unwrap();
        let stage = tempfile::Builder::new()
            .prefix(STAGE_PREFIX)
            .rand_bytes(STAGE_SUFFIX_LEN)
            .tempdir_in(parent.path())
            .unwrap();
        let target = parent.path().join("xscreen Türkçe test.exe");
        fs::write(&target, b"old").unwrap();
        fs::write(stage.path().join(PAYLOAD), b"new").unwrap();
        (parent, stage, target)
    }

    #[test]
    fn portable_update_replaces_original_at_the_same_path() {
        let (_parent, stage, target) = fixture();
        assert!(is_stage_dir(stage.path()));
        replace_executable(stage.path(), &target, || Ok(())).unwrap();
        assert_eq!(fs::read(target).unwrap(), b"new");
        assert_eq!(fs::read(stage.path().join(BACKUP)).unwrap(), b"old");
    }

    #[test]
    fn failed_launch_rolls_back_and_missing_payload_does_not_replace() {
        let (_parent, stage, target) = fixture();
        let failure = replace_executable(stage.path(), &target, || Err(io::Error::other("launch")))
            .unwrap_err();
        assert!(failure.backup.is_none());
        assert_eq!(fs::read(&target).unwrap(), b"old");
        assert!(replace_executable(stage.path(), &target, || panic!("must not launch")).is_err());
        assert_eq!(fs::read(target).unwrap(), b"old");
    }

    #[test]
    fn helper_paths_are_limited_to_one_filename() {
        assert!(single_name(OsStr::new("Türkçe test.exe")));
        for path in [
            r"C:\xscreen.exe",
            r"C:xscreen.exe",
            r"..\xscreen.exe",
            "xscreen.exe:stream",
            ".",
            "xscreen.exe/",
        ] {
            assert!(!single_name(OsStr::new(path)), "{path:?}");
        }
    }
}
