use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use eframe::egui;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

mod install;
pub use install::{cleanup_after_start, helper_mode, install_after_exit};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const LATEST: &str = "https://api.github.com/repos/TheRaven815/xscreen/releases/latest";
pub const RELEASES: &str = "https://github.com/TheRaven815/xscreen/releases";
const EXE: &str = "xscreen-windows-x64.exe";
const CHECKSUM: &str = "SHA256SUMS.txt";
const MAX_EXE: u64 = 64 * 1024 * 1024;
const MAX_CHECKSUM: u64 = 4096;
const STAGE_PREFIX: &str = ".xscreen-update-";
const STAGE_SUFFIX_LEN: usize = 12;
const PAYLOAD: &str = "download.exe";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Version(u16, u16, u16);

impl Version {
    fn parse(s: &str) -> Option<Self> {
        fn part(s: &str) -> Option<u16> {
            if !s.is_empty()
                && s.bytes().all(|b| b.is_ascii_digit())
                && (s == "0" || !s.starts_with('0'))
            {
                s.parse().ok()
            } else {
                None
            }
        }
        let mut parts = s.split('.');
        let v = Self(
            part(parts.next()?)?,
            part(parts.next()?)?,
            part(parts.next()?)?,
        );
        parts.next().is_none().then_some(v)
    }
}

#[derive(Clone, Debug)]
pub struct Release {
    tag: String,
    bytes: u64,
}

impl Release {
    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub fn notes_url(&self) -> String {
        format!("{RELEASES}/tag/{}", self.tag)
    }

    fn asset_url(&self, name: &str) -> String {
        format!("{RELEASES}/download/{}/{name}", self.tag)
    }
}

#[derive(Deserialize)]
struct FeedRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<FeedAsset>,
}

#[derive(Deserialize)]
struct FeedAsset {
    name: String,
    size: u64,
    state: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Network,
    RateLimited,
    Release,
    Integrity,
    Storage,
    Worker,
}

pub(crate) enum Checked {
    NoRelease,
    Current,
    Available(Release),
}

pub struct Staged {
    dir: TempDir,
    target: PathBuf,
    release: Release,
    sha256: String,
}

impl Staged {
    pub fn release(&self) -> &Release {
        &self.release
    }
}

type Job<T> = mpsc::Receiver<Result<T, Failure>>;

#[derive(Default)]
pub enum Updater {
    #[default]
    Idle,
    Checking(Job<Checked>),
    NoRelease,
    Current,
    CheckFailed(Failure),
    Available(Release, Option<Failure>),
    Downloading(Release, Job<Staged>),
    Ready(Staged),
}

impl Updater {
    pub fn check(&mut self, ctx: &egui::Context) {
        if matches!(
            self,
            Self::Checking(_) | Self::Downloading(_, _) | Self::Ready(_)
        ) {
            return;
        }
        *self = match background(ctx, check) {
            Ok(job) => Self::Checking(job),
            Err(failure) => Self::CheckFailed(failure),
        };
    }

    pub fn download(&mut self, ctx: &egui::Context) {
        let Self::Available(release, _) = self else {
            return;
        };
        let candidate = release.clone();
        match background(ctx, move || download(candidate)) {
            Ok(job) => *self = Self::Downloading(release.clone(), job),
            Err(failure) => *self = Self::Available(release.clone(), Some(failure)),
        }
    }

    pub fn take_ready(&mut self) -> Option<Staged> {
        if !matches!(self, Self::Ready(_)) {
            return None;
        }
        match std::mem::take(self) {
            Self::Ready(staged) => Some(staged),
            _ => unreachable!(),
        }
    }

    pub fn cancel_ready(&mut self) {
        if let Self::Ready(staged) = self {
            *self = Self::Available(staged.release().clone(), None);
        }
    }

    pub fn poll(&mut self) {
        fn receive<T>(job: &Job<T>) -> Option<Result<T, Failure>> {
            match job.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(Failure::Worker)),
            }
        }
        let next = match self {
            Self::Checking(job) => receive(job).map(|result| match result {
                Ok(Checked::NoRelease) => Self::NoRelease,
                Ok(Checked::Current) => Self::Current,
                Ok(Checked::Available(release)) => Self::Available(release, None),
                Err(failure) => Self::CheckFailed(failure),
            }),
            Self::Downloading(release, job) => receive(job).map(|result| match result {
                Ok(staged) => Self::Ready(staged),
                Err(failure) => Self::Available(release.clone(), Some(failure)),
            }),
            _ => None,
        };
        if let Some(next) = next {
            *self = next;
        }
    }
}

fn background<T: Send + 'static>(
    ctx: &egui::Context,
    task: impl FnOnce() -> Result<T, Failure> + Send + 'static,
) -> Result<Job<T>, Failure> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::Builder::new()
        .name("xscreen-update".into())
        .spawn(move || {
            let _ = tx.send(task());
            ctx.request_repaint();
        })
        .map_err(|error| {
            tracing::error!(%error, "update thread could not start");
            Failure::Worker
        })?;
    Ok(rx)
}

fn agent(timeout: u64, redirects: u32) -> Agent {
    Agent::config_builder()
        .https_only(true)
        .http_status_as_error(false)
        .max_redirects(redirects)
        .timeout_global(Some(Duration::from_secs(timeout)))
        .user_agent("xscreen-updater")
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into()
}

fn get(
    agent: &Agent,
    url: &str,
    accept: &str,
) -> Result<ureq::http::Response<ureq::Body>, Failure> {
    agent
        .get(url)
        .header("Accept", accept)
        .call()
        .map_err(|error| {
            // Do not log signed asset URLs or proxy details.
            tracing::error!(kind = ?std::mem::discriminant(&error), "update HTTP request failed");
            Failure::Network
        })
}

fn http_failure(status: u16) -> Failure {
    tracing::error!(status, "unexpected update HTTP response");
    match status {
        403 | 429 => Failure::RateLimited,
        404 | 410 => Failure::Release,
        _ => Failure::Network,
    }
}

fn bounded_body(mut reader: impl Read, max: u64) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    (&mut reader)
        .take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure::Network)?;
    if bytes.len() as u64 > max {
        return Err(Failure::Release);
    }
    Ok(bytes)
}

fn check() -> Result<Checked, Failure> {
    let response = get(&agent(15, 0), LATEST, "application/vnd.github+json")?;
    match response.status().as_u16() {
        200 => {
            let bytes = bounded_body(response.into_body().into_reader(), 1024 * 1024)?;
            parse_release(
                &bytes,
                Version::parse(CURRENT_VERSION).ok_or(Failure::Release)?,
            )
        }
        404 => Ok(Checked::NoRelease),
        status => Err(http_failure(status)),
    }
}

fn parse_release(bytes: &[u8], current: Version) -> Result<Checked, Failure> {
    let feed: FeedRelease = serde_json::from_slice(bytes).map_err(|_| Failure::Release)?;
    if feed.draft || feed.prerelease {
        return Ok(Checked::NoRelease);
    }
    let version = feed
        .tag_name
        .strip_prefix('v')
        .and_then(Version::parse)
        .ok_or(Failure::Release)?;
    if version <= current {
        return Ok(Checked::Current);
    }

    let size = |name, max| {
        let mut assets = feed.assets.iter().filter(|a| a.name == name);
        let asset = assets.next().ok_or(Failure::Release)?;
        if assets.next().is_some() || asset.state != "uploaded" || !(1..=max).contains(&asset.size)
        {
            return Err(Failure::Release);
        }
        Ok(asset.size)
    };
    let bytes = size(EXE, MAX_EXE)?;
    size(CHECKSUM, MAX_CHECKSUM)?;
    Ok(Checked::Available(Release {
        tag: feed.tag_name,
        bytes,
    }))
}

fn asset(agent: &Agent, release: &Release, name: &str) -> Result<impl Read + use<>, Failure> {
    let response = get(agent, &release.asset_url(name), "application/octet-stream")?;
    match response.status().as_u16() {
        200 => Ok(response.into_body().into_reader()),
        status => Err(http_failure(status)),
    }
}

fn parse_checksum(bytes: &[u8]) -> Result<String, Failure> {
    let text = std::str::from_utf8(bytes).map_err(|_| Failure::Integrity)?;
    let mut sums = text.lines().filter_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        let name = name.trim_start();
        (name == EXE || name.strip_prefix('*') == Some(EXE)).then_some(hash)
    });
    let hash = sums.next().ok_or(Failure::Integrity)?;
    if sums.next().is_some() || !is_sha256(hash) {
        return Err(Failure::Integrity);
    }
    Ok(hash.to_ascii_lowercase())
}

fn is_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

fn storage(error: io::Error) -> Failure {
    tracing::error!(%error, "update staging failed");
    Failure::Storage
}

fn download(release: Release) -> Result<Staged, Failure> {
    let target = std::env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(storage)?;
    let folder = target.parent().ok_or(Failure::Storage)?;
    // Same volume as the running binary, so install can replace it with one rename.
    let dir = tempfile::Builder::new()
        .prefix(STAGE_PREFIX)
        .rand_bytes(STAGE_SUFFIX_LEN)
        .tempdir_in(folder)
        .map_err(storage)?;
    let agent = agent(90, 4);
    let checksum = asset(&agent, &release, CHECKSUM)?;
    let sha256 = parse_checksum(&bounded_body(checksum, MAX_CHECKSUM)?)?;
    let mut source = asset(&agent, &release, EXE)?;
    let mut dest = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(dir.path().join(PAYLOAD))
        .map_err(storage)?;
    write_verified(&mut source, &mut dest, release.bytes, &sha256)?;
    Ok(Staged {
        dir,
        target,
        release,
        sha256,
    })
}

fn write_verified(
    mut source: impl Read,
    dest: &mut File,
    size: u64,
    expected: &str,
) -> Result<(), Failure> {
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    let mut buf = [0; 32 * 1024];
    loop {
        let n = source.read(&mut buf).map_err(|_| Failure::Network)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > size {
            return Err(Failure::Integrity);
        }
        hash.update(&buf[..n]);
        dest.write_all(&buf[..n]).map_err(storage)?;
    }
    if bytes != size || format!("{:x}", hash.finalize()) != expected {
        return Err(Failure::Integrity);
    }
    if !is_windows_x64_exe(dest, bytes).map_err(storage)? {
        return Err(Failure::Integrity);
    }
    dest.sync_all().map_err(storage)
}

fn is_windows_x64_exe(file: &mut (impl Read + Seek), len: u64) -> io::Result<bool> {
    let mut dos = [0; 64];
    if len < dos.len() as u64 {
        return Ok(false);
    }
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut dos)?;
    let pe = u32::from_le_bytes(dos[60..64].try_into().unwrap()) as u64;
    if &dos[..2] != b"MZ" || pe < 64 || pe + 26 > len {
        return Ok(false);
    }
    let mut header = [0; 26];
    file.seek(SeekFrom::Start(pe))?;
    file.read_exact(&mut header)?;
    let flags = u16::from_le_bytes([header[22], header[23]]);
    Ok(
        &header[..6] == b"PE\0\0\x64\x86" // IMAGE_FILE_MACHINE_AMD64
        && header[24..26] == [0x0b, 0x02] // PE32+
        && flags & 0x2002 == 0x0002,
    ) // executable, not a DLL
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn feed(tag: &str) -> serde_json::Value {
        json!({
            "tag_name": tag,
            "draft": false,
            "prerelease": false,
            "assets": [
                {"name": EXE, "size": 1024, "state": "uploaded", "browser_download_url": "https://other.invalid/payload.exe"},
                {"name": CHECKSUM, "size": 90, "state": "uploaded"}
            ]
        })
    }

    fn classify(feed: serde_json::Value) -> Result<Checked, Failure> {
        parse_release(&serde_json::to_vec(&feed).unwrap(), Version(0, 2, 0))
    }

    #[test]
    #[ignore = "contacts the public GitHub Release API"]
    fn live_release_feed() {
        match check().expect("GitHub Release API request failed") {
            Checked::NoRelease => eprintln!("No stable GitHub release yet."),
            Checked::Current => eprintln!("Installed version is current."),
            Checked::Available(release) => eprintln!("Available: {}", release.tag()),
        }
    }

    #[test]
    fn release_tags_compare_numerically_and_cannot_choose_download_hosts() {
        assert!(matches!(classify(feed("v0.2.0")), Ok(Checked::Current)));
        assert!(matches!(classify(feed("v0.1.99")), Ok(Checked::Current)));
        let Ok(Checked::Available(release)) = classify(feed("v0.10.0")) else {
            panic!("expected newer release");
        };
        assert_eq!(release.tag(), "v0.10.0");
        assert_eq!(release.bytes, 1024);
        assert_eq!(
            release.asset_url(EXE),
            format!("{RELEASES}/download/v0.10.0/{EXE}")
        );
        for tag in [
            "V1.0.0",
            "v1.0",
            "v1.0.0-rc.1",
            "v1.00.0",
            "v65536.0.0",
            "v1.0.0/other",
        ] {
            assert!(
                matches!(classify(feed(tag)), Err(Failure::Release)),
                "{tag}"
            );
        }
    }

    #[test]
    fn refuses_drafts_prereleases_and_incomplete_packages() {
        for field in ["draft", "prerelease"] {
            let mut f = feed("v1.0.0");
            f[field] = json!(true);
            assert!(matches!(classify(f), Ok(Checked::NoRelease)));
        }
        let mut f = feed("v1.0.0");
        f["assets"][0]["size"] = json!(MAX_EXE + 1);
        assert!(matches!(classify(f), Err(Failure::Release)));
        let mut f = feed("v1.0.0");
        f["assets"][1]["name"] = json!(EXE);
        assert!(matches!(classify(f), Err(Failure::Release)));
        let mut f = feed("v1.0.0");
        f["assets"][0]["state"] = json!("starter");
        assert!(matches!(classify(f), Err(Failure::Release)));
    }

    #[test]
    fn checksum_must_identify_exactly_one_x64_asset() {
        let hash = "AF".repeat(32);
        let line = format!("{hash}  {EXE}\r\n");
        assert_eq!(parse_checksum(line.as_bytes()), Ok(hash.to_lowercase()));
        for bad in [
            format!("{line}{line}"),
            format!("{hash}  renamed.exe\n"),
            format!("{}  {EXE}\n", "Z".repeat(64)),
        ] {
            assert_eq!(parse_checksum(bad.as_bytes()), Err(Failure::Integrity));
        }
    }

    #[test]
    fn rejects_wrong_hash_size_and_architecture_before_staging() {
        let mut exe = [0u8; 128];
        exe[..2].copy_from_slice(b"MZ");
        exe[60] = 64;
        exe[64..70].copy_from_slice(b"PE\0\0\x64\x86");
        exe[86] = 2;
        exe[88..90].copy_from_slice(&[0x0b, 0x02]);
        let verify = |data: &[u8], len, hash: &str| {
            write_verified(data, &mut tempfile::tempfile().unwrap(), len, hash)
        };
        let hash = format!("{:x}", Sha256::digest(exe));
        assert_eq!(verify(&exe, 128, &hash), Ok(()));
        assert_eq!(verify(&exe, 127, &hash), Err(Failure::Integrity));
        assert_eq!(verify(&exe, 129, &hash), Err(Failure::Integrity));
        assert_eq!(verify(&exe, 128, &"0".repeat(64)), Err(Failure::Integrity));
        exe[68..70].copy_from_slice(&[0x4c, 0x01]); // x86
        assert_eq!(
            verify(&exe, 128, &format!("{:x}", Sha256::digest(exe))),
            Err(Failure::Integrity)
        );
    }
}
