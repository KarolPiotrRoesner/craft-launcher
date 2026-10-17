//! Clones of the Craft Apps' source code repos, kept with the system's own `git`.
//!
//! Each app is cloned from `https://github.com/storytold/<app>.git` into `<root>/<app>`, with its
//! full history. Updating fetches and then fast-forwards only: a clone with uncommitted changes,
//! local commits, a detached HEAD or another `origin` is fetched at most and otherwise left alone.
//! Nothing here deletes files or runs anything from a clone.

use std::path::{Path, PathBuf};
use std::time::Duration;

use artcraft_launcher_core::CraftApp;
use artcraft_launcher_core::catalog::GITHUB_OWNER;
use artcraft_launcher_core::protocol::{RepoInfo, RepoState};

use crate::process;

/// Local git commands (status, rev-parse…) finish in moments.
const LOCAL: Duration = Duration::from_secs(60);
/// A fetch over a slow connection.
const FETCH: Duration = Duration::from_secs(30 * 60);
/// A full clone over a slow connection.
const CLONE: Duration = Duration::from_secs(2 * 60 * 60);

/// Where the repos go unless the user picks a folder: `Documents/Craft Repos`.
pub fn default_root() -> PathBuf {
    dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(std::env::temp_dir).join("Craft Repos")
}

/// The folder `app` is cloned into.
pub fn checkout(root: &Path, app: &CraftApp) -> PathBuf {
    root.join(app.id)
}

/// The only address an app is cloned from: its repo under [`GITHUB_OWNER`] on GitHub. `None`
/// for a catalog row that names another owner or an odd repo name.
pub fn clone_url(app: &CraftApp) -> Option<String> {
    let (owner, name) = app.repo.split_once('/')?;
    let plain = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) && !name.starts_with('.');
    (owner == GITHUB_OWNER && plain).then(|| format!("https://github.com/{owner}/{name}.git"))
}

/// A remote URL in one comparable form: `git@github.com:o/r.git`, `ssh://git@github.com/o/r`
/// and `https://github.com/o/r/` all become `https://github.com/o/r`.
fn normalize(url: &str) -> String {
    let mut u = url.trim().to_ascii_lowercase();
    for prefix in ["git@github.com:", "ssh://git@github.com/", "git://github.com/", "http://github.com/"] {
        if let Some(rest) = u.strip_prefix(prefix) {
            u = format!("https://github.com/{rest}");
        }
    }
    let u = u.trim_end_matches('/');
    u.strip_suffix(".git").unwrap_or(u).to_string()
}

/// Runs `git` (in `dir`, when given) without a console window, a credential prompt or a pager.
fn git(dir: Option<&Path>, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut cmd = process::command("git");
    cmd.env("GIT_TERMINAL_PROMPT", "0").env("GCM_INTERACTIVE", "never").env("GIT_PAGER", "cat").env("LC_ALL", "C");
    if cfg!(windows) {
        cmd.args(["-c", "core.longpaths=true"]);
    }
    if let Some(dir) = dir {
        cmd.arg("-C").arg(dir);
    }
    cmd.args(args);
    process::run_command(cmd, "git", timeout).map(|out| out.trim().to_string())
}

/// Whether `git` can be started.
pub fn git_available() -> bool {
    git(None, &["--version"], LOCAL).is_ok()
}

/// Where the clone at `path` stands, without touching the network.
pub fn status(path: &Path, url: &str) -> RepoState {
    if !path.exists() {
        return RepoState::NotCloned;
    }
    if !path.join(".git").exists() {
        return RepoState::NotARepo;
    }
    match git(Some(path), &["remote", "get-url", "origin"], LOCAL) {
        Ok(origin) if normalize(&origin) == normalize(url) => {}
        Ok(_) => return RepoState::ForeignRemote,
        Err(_) if !git_available() => return RepoState::GitMissing,
        Err(_) => return RepoState::ForeignRemote,
    }
    RepoState::Ready(info(path))
}

fn info(path: &Path) -> RepoInfo {
    let branch = git(Some(path), &["symbolic-ref", "--short", "-q", "HEAD"], LOCAL).ok().filter(|b| !b.is_empty());
    // An empty repo has no HEAD commit yet.
    let head = git(Some(path), &["rev-parse", "--short", "HEAD"], LOCAL).unwrap_or_default();
    let modified = git(Some(path), &["status", "--porcelain"], LOCAL).is_ok_and(|s| !s.is_empty());
    // "<ahead>\t<behind>"; fails without an upstream branch.
    let (ahead, behind) = git(Some(path), &["rev-list", "--left-right", "--count", "HEAD...@{u}"], LOCAL)
        .ok()
        .and_then(|out| {
            let mut parts = out.split_whitespace().map(str::parse::<u32>);
            Some((parts.next()?.ok()?, parts.next()?.ok()?))
        })
        .unwrap_or((0, 0));
    let date = git(Some(path), &["log", "-1", "--format=%cs"], LOCAL).ok().filter(|d| !d.is_empty());
    RepoInfo { branch, head, behind, ahead, modified, version: cargo_version(path), date }
}

/// The program version a clone's `Cargo.toml` declares: `[workspace.package] version`, else
/// `[package] version`. Read line by line (no TOML parser), at most 1 MB.
pub fn cargo_version(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(path.join("Cargo.toml")).ok()?.take(1 << 20).read_to_string(&mut text).ok()?;
    let mut section = String::new();
    let (mut workspace, mut package) = (None, None);
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']').trim().to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.trim() != "version" {
            continue;
        }
        // `version = "0.6.0"`, maybe followed by a comment; `version.workspace = true` has no quotes.
        let value = value.trim();
        let Some(rest) = value.strip_prefix('"') else { continue };
        let Some(version) = rest.split('"').next().filter(|v| !v.is_empty() && v.len() <= 64) else { continue };
        match section.as_str() {
            "workspace.package" => workspace = workspace.or(Some(version.to_string())),
            "package" => package = package.or(Some(version.to_string())),
            _ => {}
        }
    }
    workspace.or(package)
}

/// What a repo job does with a clone that's already there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Clone it if it's missing, otherwise fetch and fast-forward.
    Sync,
    /// Fetch only, so the clone knows what's waiting; a missing clone stays missing.
    Check,
}

/// Brings the clone of `name` at `path` (from `url`) up to date as `mode` says, reporting each
/// step through `step`. Returns where it ends and a short message: `Err` when it failed or the
/// clone was left alone.
pub fn sync(name: &str, path: &Path, url: &str, mode: Mode, step: impl Fn(RepoState)) -> (RepoState, Result<String, String>) {
    if !git_available() {
        return (RepoState::GitMissing, Err("Git isn't installed. Install it from git-scm.com, then try again.".to_string()));
    }
    match status(path, url) {
        RepoState::NotCloned if mode == Mode::Check => (RepoState::NotCloned, Ok(String::new())),
        RepoState::NotCloned => clone(name, path, url, step),
        RepoState::NotARepo => {
            (RepoState::NotARepo, Err(format!("{} already exists and isn't a git repository, so it was left alone", path.display())))
        }
        RepoState::ForeignRemote => {
            (RepoState::ForeignRemote, Err(format!("{} is a clone of another repository, so it was left alone", path.display())))
        }
        RepoState::Ready(_) => {
            step(RepoState::Fetching);
            if let Err(e) = git(Some(path), &["fetch", "--prune", "--tags", "origin"], FETCH) {
                return (status(path, url), Err(format!("Couldn't fetch {name}'s source code: {e}")));
            }
            let info = info(path);
            let result = match mode {
                Mode::Check => Ok(String::new()),
                Mode::Sync => fast_forward(name, path, &info),
            };
            (status(path, url), result)
        }
        other => (other, Err(format!("Couldn't look at {}", path.display()))),
    }
}

fn clone(name: &str, path: &Path, url: &str, step: impl Fn(RepoState)) -> (RepoState, Result<String, String>) {
    step(RepoState::Cloning);
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return (RepoState::NotCloned, Err(format!("Couldn't create {}: {e}", parent.display())));
    }
    let Some(target) = path.to_str() else {
        return (RepoState::NotCloned, Err(format!("{} isn't a usable path", path.display())));
    };
    // git removes a clone it couldn't finish.
    match git(None, &["clone", "--", url, target], CLONE) {
        Ok(_) => (status(path, url), Ok(format!("{name}'s source code was cloned"))),
        Err(e) => (status(path, url), Err(format!("Couldn't clone {name}'s source code: {e}"))),
    }
}

fn fast_forward(name: &str, path: &Path, info: &RepoInfo) -> Result<String, String> {
    if info.behind == 0 {
        return Ok(format!("{name}'s source code is up to date"));
    }
    if info.branch.is_none() {
        return Err(format!("{name}'s clone isn't on a branch, so it was only fetched"));
    }
    if info.ahead > 0 {
        return Err(format!("{name}'s clone has local commits that aren't on GitHub, so it was only fetched"));
    }
    if info.modified {
        return Err(format!("{name}'s clone has local changes, so it was only fetched"));
    }
    git(Some(path), &["merge", "--ff-only", "--quiet", "@{u}"], LOCAL)
        .map(|_| format!("{name}'s source code updated ({} new commits)", info.behind))
        .map_err(|e| format!("Couldn't update {name}'s source code: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repo with one commit to clone from: `(<dir>, <its url>)`.
    fn origin(dir: &Path) -> String {
        let src = dir.join("origin");
        std::fs::create_dir_all(&src).unwrap();
        for args in [&["init", "-q", "-b", "main"][..], &["config", "user.email", "test@example.invalid"], &["config", "user.name", "Test"]] {
            git(Some(&src), args, LOCAL).unwrap();
        }
        commit(&src, "a.txt", "one");
        src.to_str().unwrap().to_string()
    }

    fn commit(repo: &Path, file: &str, text: &str) {
        std::fs::write(repo.join(file), text).unwrap();
        git(Some(repo), &["add", "."], LOCAL).unwrap();
        git(Some(repo), &["-c", "user.email=test@example.invalid", "-c", "user.name=Test", "commit", "-q", "-m", text], LOCAL).unwrap();
    }

    fn ready(state: RepoState) -> RepoInfo {
        match state {
            RepoState::Ready(info) => info,
            other => panic!("expected a clone, got {other:?}"),
        }
    }

    #[test]
    fn the_version_comes_from_the_workspace_or_the_package() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(cargo_version(dir.path()), None);
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\nversion = \"0.3.1\" # released\n").unwrap();
        assert_eq!(cargo_version(dir.path()).as_deref(), Some("0.3.1"));
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nversion.workspace = true\n\n[workspace.package]\nedition = \"2024\"\nversion = \"0.6.0\"\n[dependencies]\nserde = { version = \"1\" }\n",
        )
        .unwrap();
        assert_eq!(cargo_version(dir.path()).as_deref(), Some("0.6.0"));
        std::fs::write(dir.path().join("Cargo.toml"), "[dependencies]\nversion = \"9\"\n").unwrap();
        assert_eq!(cargo_version(dir.path()), None);
    }

    #[test]
    fn urls_compare_in_one_form() {
        let want = normalize("https://github.com/storytold/photocraft.git");
        for url in [
            "git@github.com:storytold/photocraft.git",
            "ssh://git@github.com/storytold/photocraft",
            "https://github.com/Storytold/PhotoCraft/",
            "https://github.com/storytold/photocraft",
        ] {
            assert_eq!(normalize(url), want, "{url}");
        }
        assert_ne!(normalize("https://github.com/someone/photocraft"), want);
    }

    #[test]
    fn apps_are_cloned_from_storytold_only() {
        let photo = CraftApp::by_id("photocraft").unwrap();
        assert_eq!(clone_url(photo).as_deref(), Some("https://github.com/storytold/photocraft.git"));
        let elsewhere = CraftApp { repo: "someone/photocraft", ..*photo };
        assert_eq!(clone_url(&elsewhere), None);
        let odd = CraftApp { repo: "storytold/../x", ..*photo };
        assert_eq!(clone_url(&odd), None);
    }

    #[test]
    fn a_missing_folder_is_not_cloned_and_a_plain_folder_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let url = "https://github.com/storytold/photocraft.git";
        assert_eq!(status(&dir.path().join("photocraft"), url), RepoState::NotCloned);
        std::fs::create_dir(dir.path().join("photocraft")).unwrap();
        std::fs::write(dir.path().join("photocraft/keep.txt"), "mine").unwrap();
        let (state, result) = sync("PhotoCraft", &dir.path().join("photocraft"), url, Mode::Sync, |_| {});
        assert_eq!(state, RepoState::NotARepo);
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(dir.path().join("photocraft/keep.txt")).unwrap(), "mine");
    }

    #[test]
    fn clone_check_and_fast_forward() {
        if !git_available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let url = origin(dir.path());
        let path = dir.path().join("repos/photocraft");

        // A check leaves a missing clone missing.
        assert_eq!(sync("PhotoCraft", &path, &url, Mode::Check, |_| {}).0, RepoState::NotCloned);

        let steps = std::cell::RefCell::new(Vec::new());
        let (state, result) = sync("PhotoCraft", &path, &url, Mode::Sync, |s| steps.borrow_mut().push(s));
        assert_eq!(result, Ok("PhotoCraft's source code was cloned".to_string()));
        assert_eq!(steps.into_inner(), vec![RepoState::Cloning]);
        let info = ready(state);
        assert_eq!((info.branch.as_deref(), info.behind, info.ahead, info.modified), (Some("main"), 0, 0, false));
        assert_eq!(info.date.map(|d| d.len()), Some(10));
        assert_eq!(info.version, None);

        // Two new commits upstream: a check counts them and changes nothing.
        let src = Path::new(&url);
        commit(src, "b.txt", "two");
        commit(src, "c.txt", "three");
        let (state, result) = sync("PhotoCraft", &path, &url, Mode::Check, |_| {});
        assert!(result.is_ok());
        assert_eq!(ready(state).behind, 2);
        assert!(!path.join("b.txt").exists());

        // An update fast-forwards.
        let (state, result) = sync("PhotoCraft", &path, &url, Mode::Sync, |_| {});
        assert_eq!(result, Ok("PhotoCraft's source code updated (2 new commits)".to_string()));
        assert_eq!(ready(state).behind, 0);
        assert!(path.join("c.txt").exists());
        assert_eq!(sync("PhotoCraft", &path, &url, Mode::Sync, |_| {}).1, Ok("PhotoCraft's source code is up to date".to_string()));
    }

    #[test]
    fn local_work_is_never_overwritten() {
        if !git_available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let url = origin(dir.path());
        let path = dir.path().join("photocraft");
        assert!(sync("PhotoCraft", &path, &url, Mode::Sync, |_| {}).1.is_ok());
        commit(Path::new(&url), "b.txt", "two");

        // Uncommitted changes: fetched, not merged.
        std::fs::write(path.join("a.txt"), "edited").unwrap();
        let (state, result) = sync("PhotoCraft", &path, &url, Mode::Sync, |_| {});
        assert!(result.unwrap_err().contains("local changes"));
        let info = ready(state);
        assert!(info.modified);
        assert_eq!(info.behind, 1);
        assert_eq!(std::fs::read_to_string(path.join("a.txt")).unwrap(), "edited");

        // A local commit on top: diverged, left alone.
        commit(&path, "a.txt", "mine");
        let (state, result) = sync("PhotoCraft", &path, &url, Mode::Sync, |_| {});
        assert!(result.unwrap_err().contains("local commits"));
        let info = ready(state);
        assert_eq!((info.ahead, info.behind), (1, 1));
        assert!(!path.join("b.txt").exists());
    }

    #[test]
    fn a_clone_of_another_repo_is_left_alone() {
        if !git_available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let url = origin(dir.path());
        let path = dir.path().join("photocraft");
        assert!(sync("PhotoCraft", &path, &url, Mode::Sync, |_| {}).1.is_ok());
        let (state, result) = sync("PhotoCraft", &path, "https://github.com/storytold/photocraft.git", Mode::Sync, |_| {});
        assert_eq!(state, RepoState::ForeignRemote);
        assert!(result.is_err());
    }

    /// Clones a real app from GitHub (`cargo test -p artcraft-launcher-native -- --ignored real`).
    #[test]
    #[ignore = "needs the network and git"]
    fn clones_a_real_app_from_github() {
        let app = CraftApp::by_id("figcraft").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = checkout(dir.path(), app);
        let url = clone_url(app).unwrap();
        let (state, result) = sync(app.name, &path, &url, Mode::Sync, |_| {});
        assert_eq!(result, Ok("FigCraft's source code was cloned".to_string()));
        assert!(!ready(state).head.is_empty());
        let (state, result) = sync(app.name, &path, &url, Mode::Check, |_| {});
        assert!(result.is_ok());
        assert_eq!(ready(state).behind, 0);
    }
}
