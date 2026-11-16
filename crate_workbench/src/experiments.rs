//! Persistent named experiments. Each operation holds an advisory lock.
use super::*;
use std::path::PathBuf;

pub const HELP: &str = "EXPERIMENTS\n:new NAME       Create and select a saved experiment\n:use NAME       Open an existing experiment\n:add serde:1, serde_json:1\n:remove serde   Remove a dependency\n:show           Show selection and last result\n:list           List experiments\n:build          Build selected experiment\n:graph          Analyze selected experiment\n:help           This help\n\nCommands work from any plugin. Changes are saved automatically.\nBuild and graph reuse Cargo.lock; changing dependencies invalidates it.";

pub struct Store {
    root: PathBuf,
}
struct StoreLock(fs::File);
impl Drop for StoreLock {
    fn drop(&mut self) {
        // Explicit unlock also releases briefly inherited descriptors during fork/exec.
        let _ = self.0.unlock();
    }
}
impl Store {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn default_location() -> Result<Self, String> {
        let root = if let Some(path) = std::env::var_os("XCRATESX_DATA_DIR") {
            PathBuf::from(path)
        } else if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
            PathBuf::from(path).join("xcratesx")
        } else {
            PathBuf::from(std::env::var_os("HOME").ok_or("Set XCRATESX_DATA_DIR or HOME")?)
                .join(".local/share/xcratesx")
        };
        Ok(Self::new(root))
    }
    fn lock(&self) -> Result<StoreLock, String> {
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join(".lock"))
            .map_err(|e| e.to_string())?;
        file.try_lock().map_err(|_| {
            "Another experiment operation is running. Try again when it finishes.".to_string()
        })?;
        Ok(StoreLock(file))
    }
    fn name(name: &str) -> Result<(), String> {
        if name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Use a name of 1–64 ASCII letters, digits, underscores or hyphens.".into());
        }
        Ok(())
    }
    fn dir(&self, name: &str) -> Result<PathBuf, String> {
        Self::name(name)?;
        let dir = self.root.join(name);
        if dir.is_symlink() {
            return Err("Symlink experiment directories are not supported.".into());
        }
        Ok(dir)
    }
    fn current(&self) -> Result<PathBuf, String> {
        let name = fs::read_to_string(self.root.join("active"))
            .map_err(|_| "No experiment selected. Use :new NAME or :use NAME.")?;
        self.dir(name.trim())
    }
    fn atomic(path: &Path, content: &str) -> Result<(), String> {
        let mut temp = tempfile::NamedTempFile::new_in(path.parent().ok_or("Missing parent")?)
            .map_err(|e| e.to_string())?;
        temp.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        temp.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn deps(dir: &Path) -> Result<BTreeMap<String, String>, String> {
        let raw = fs::read_to_string(dir.join("dependencies.toml")).map_err(|e| e.to_string())?;
        toml::from_str(&raw).map_err(|e| e.to_string())
    }
    fn write_deps(dir: &Path, deps: &BTreeMap<String, String>) -> Result<(), String> {
        let generated = project(deps)?;
        fs::create_dir_all(dir.join("src")).map_err(|e| e.to_string())?;
        for relative in ["Cargo.toml", "src/lib.rs"] {
            let text =
                fs::read_to_string(generated.path().join(relative)).map_err(|e| e.to_string())?;
            Self::atomic(&dir.join(relative), &text)?;
        }
        // Invalidate old build results before changing requirements.
        for name in ["Cargo.lock", "result.txt", "graph.txt"] {
            let path = dir.join(name);
            if path.exists() {
                fs::remove_file(path).map_err(|e| e.to_string())?;
            }
        }
        Self::atomic(
            &dir.join("dependencies.toml"),
            &toml::to_string(deps).map_err(|e| e.to_string())?,
        )
    }
    pub fn command(&self, command: &str) -> Result<String, String> {
        let _lock = self.lock()?;
        let (verb, arg) = command
            .trim()
            .split_once(' ')
            .unwrap_or((command.trim(), ""));
        let arg = arg.trim();
        match verb {
            ":help" => Ok(HELP.into()),
            ":new" => {
                let dir = self.dir(arg)?;
                fs::create_dir(&dir).map_err(|e| format!("Cannot create experiment {arg}: {e}"))?;
                Self::write_deps(&dir, &BTreeMap::new())?;
                Self::atomic(&self.root.join("active"), arg)?;
                Ok(format!("Created {arg}. Next: :add serde:1, serde_json:1"))
            }
            ":use" => {
                let dir = self.dir(arg)?;
                Self::deps(&dir)?;
                Self::atomic(&self.root.join("active"), arg)?;
                Ok(format!("Selected {arg}. Use :show, :build or :graph."))
            }
            ":list" => {
                let mut names = fs::read_dir(&self.root)
                    .map_err(|e| e.to_string())?
                    .filter_map(Result::ok)
                    .filter(|e| e.path().join("dependencies.toml").is_file())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                names.sort();
                Ok(if names.is_empty() {
                    "No saved experiments. Use :new NAME.".into()
                } else {
                    names.join("\n")
                })
            }
            ":show" => {
                let dir = self.current()?;
                let deps = Self::deps(&dir)?;
                let result = fs::read_to_string(dir.join("result.txt"))
                    .unwrap_or_else(|_| "Not built since last edit.".into());
                Ok(format!(
                    "Experiment: {}\n\n{}\n\n{result}",
                    dir.display(),
                    toml::to_string(&deps).map_err(|e| e.to_string())?
                ))
            }
            ":add" | ":remove" => {
                let dir = self.current()?;
                let mut deps = Self::deps(&dir)?;
                if verb == ":add" {
                    deps.extend(dependencies(arg)?);
                    if deps.len() > 32 {
                        return Err("Maximum 32 dependencies.".into());
                    }
                } else if deps.remove(arg).is_none() {
                    return Err(format!("Dependency not found: {arg}"));
                }
                Self::write_deps(&dir, &deps)?;
                Ok(format!(
                    "Saved {} dependencies. Next: :build or :graph.",
                    deps.len()
                ))
            }
            _ => Err(format!("Unknown command: {verb}\n\n{HELP}")),
        }
    }
    pub fn run(&self, analyze: bool) -> Result<String, String> {
        let _lock = self.lock()?;
        let saved = self.current()?;
        let deps = Self::deps(&saved)?;
        if deps.is_empty() {
            return Err("The experiment is empty. Use :add serde:1.".into());
        }
        let project = project(&deps)?;
        let manifest = project.path().join("Cargo.toml");
        let lock = saved.join("Cargo.lock");
        let locked = lock.exists();
        if locked {
            fs::copy(&lock, project.path().join("Cargo.lock")).map_err(|e| e.to_string())?;
        }
        let mut args = if analyze {
            vec!["metadata", "--format-version", "1"]
        } else {
            vec!["build", "--quiet"]
        };
        if locked {
            args.push("--locked");
        }
        let result = cargo(&manifest, &args).and_then(|raw| {
            if analyze {
                graph(&raw)
            } else {
                Ok("BUILD OK: all selected dependencies compiled together.".into())
            }
        });
        if let Ok(raw) = fs::read_to_string(project.path().join("Cargo.lock")) {
            Self::atomic(&lock, &raw)?;
        }
        let text = match &result {
            Ok(text) => text.clone(),
            Err(error) => format!("ERROR: {error}"),
        };
        Self::atomic(
            &saved.join(if analyze { "graph.txt" } else { "result.txt" }),
            &text,
        )?;
        result.map(|text| {
            format!(
                "{text}\n\nSaved in {}. Cargo.lock is reused on subsequent runs.",
                saved.display()
            )
        })
    }
}
pub fn run_saved(analyze: bool) -> Result<String, String> {
    Store::default_location()?.run(analyze)
}

pub fn command(command: &str) -> Result<String, String> {
    let store = Store::default_location()?;
    match command.trim() {
        ":build" => store.run(false),
        ":graph" => store.run(true),
        other => store.command(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_editing_and_no_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().into());
        store.command(":new demo").unwrap();
        store.command(":add serde:1, serde_json:=1.0.100").unwrap();
        let reopened = Store::new(temp.path().into());
        assert!(reopened.command(":show").unwrap().contains("=1.0.100"));
        assert!(reopened.command(":new demo").is_err());
        assert!(reopened.command(":new ../escape").is_err());
        assert!(reopened.command(":use missing").is_err());
        reopened.command(":remove serde").unwrap();
        assert_eq!(Store::deps(&temp.path().join("demo")).unwrap().len(), 1);
        assert_eq!(reopened.command(":list").unwrap(), "demo");
    }
    #[test]
    fn edit_invalidates_lock_and_results() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().into());
        store.command(":new demo").unwrap();
        fs::write(temp.path().join("demo/Cargo.lock"), "old").unwrap();
        fs::write(temp.path().join("demo/result.txt"), "BUILD OK").unwrap();
        store.command(":add serde:1").unwrap();
        assert!(!temp.path().join("demo/Cargo.lock").exists());
        assert!(!store.command(":show").unwrap().contains("BUILD OK"));
    }
    #[test]
    fn concurrent_operations_fail_without_changing_selection() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().into());
        let _lock = store.lock().unwrap();
        assert!(store.command(":new blocked").is_err());
        assert!(!temp.path().join("blocked").exists());
    }
}
