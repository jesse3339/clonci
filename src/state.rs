use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const APP_NAME: &str = "clonci";
const LAST_CONTEXT_FILE: &str = "last_context";

pub fn create_context(name: &str) -> Result<(), String> {
    validate_context_name(name)?;
    ensure_state_dirs().map_err(|e| format!("failed to prepare state directory: {e}"))?;

    let dir = context_dir(name).map_err(|e| e.to_string())?;
    if dir.exists() {
        return Err(format!("context '{name}' already exists"));
    }

    fs::create_dir_all(&dir).map_err(|e| format!("failed to create context '{name}': {e}"))?;

    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("time error: {e}"))?
        .as_secs();
    let meta = format!("name={name}\ncreated_unix={created}\nversion=1\nshells=bash,zsh,pwsh\n");

    fs::write(dir.join("meta.txt"), meta)
        .map_err(|e| format!("failed to write metadata for '{name}': {e}"))?;

    Ok(())
}

pub fn list_context_names() -> Result<Vec<String>, String> {
    ensure_state_dirs().map_err(|e| format!("failed to prepare state directory: {e}"))?;

    let dir = contexts_dir().map_err(|e| e.to_string())?;
    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("failed to read contexts directory '{}': {e}", dir.display()))?;

    let mut contexts: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();

    contexts.sort();
    Ok(contexts)
}

pub fn delete_context(name: &str) -> Result<(), String> {
    validate_context_name(name)?;
    let dir = context_dir(name).map_err(|e| e.to_string())?;

    if !dir.exists() {
        return Err(format!("context '{name}' does not exist"));
    }

    fs::remove_dir_all(&dir).map_err(|e| format!("failed to delete context '{name}': {e}"))?;

    let last = read_last_context()?;
    if last.as_deref() == Some(name) {
        clear_last_context()?;
    }

    Ok(())
}

pub fn ensure_context_exists(name: &str) -> Result<(), String> {
    let path = context_dir(name).map_err(|e| e.to_string())?;
    if path.exists() {
        return Ok(());
    }

    Err(format!(
        "context '{name}' does not exist. Create it with: clonci context create {name}"
    ))
}

pub fn ensure_history_file(context: &str, history_file_name: &str) -> Result<PathBuf, String> {
    let dir = context_dir(context).map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| {
        format!(
            "failed to ensure context directory '{}': {e}",
            dir.display()
        )
    })?;

    let path = dir.join(history_file_name);
    if !path.exists() {
        fs::write(&path, b"")
            .map_err(|e| format!("failed to create history file '{}': {e}", path.display()))?;
    }

    Ok(path)
}

pub fn write_last_context(name: &str) -> Result<(), String> {
    let path = state_root()
        .map_err(|e| format!("failed to find state directory: {e}"))?
        .join(LAST_CONTEXT_FILE);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "failed to create state directory '{}': {e}",
                parent.display()
            )
        })?;
    }

    fs::write(&path, format!("{name}\n"))
        .map_err(|e| format!("failed to write '{}': {e}", path.display()))
}

pub fn read_last_context() -> Result<Option<String>, String> {
    let path = state_root()
        .map_err(|e| format!("failed to find state directory: {e}"))?
        .join(LAST_CONTEXT_FILE);

    if !path.exists() {
        return Ok(None);
    }

    let value = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read '{}': {e}", path.display()))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

pub fn validate_context_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("context name cannot be empty".to_string());
    }

    if name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        Err("context name must only contain [A-Za-z0-9_-]".to_string())
    }
}

fn clear_last_context() -> Result<(), String> {
    let path = state_root()
        .map_err(|e| format!("failed to find state directory: {e}"))?
        .join(LAST_CONTEXT_FILE);

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("failed to remove '{}': {e}", path.display()))?;
    }

    Ok(())
}

fn ensure_state_dirs() -> io::Result<()> {
    let contexts = contexts_dir()?;
    fs::create_dir_all(contexts)
}

fn contexts_dir() -> io::Result<PathBuf> {
    Ok(state_root()?.join("contexts"))
}

fn context_dir(name: &str) -> io::Result<PathBuf> {
    Ok(contexts_dir()?.join(name))
}

fn state_root() -> io::Result<PathBuf> {
    if cfg!(windows) {
        if let Ok(path) = env::var("LOCALAPPDATA") {
            if !path.trim().is_empty() {
                return Ok(PathBuf::from(path).join(APP_NAME));
            }
        }
        if let Ok(path) = env::var("USERPROFILE") {
            if !path.trim().is_empty() {
                return Ok(PathBuf::from(path).join(format!(".{APP_NAME}")));
            }
        }
    } else {
        if let Ok(path) = env::var("XDG_STATE_HOME") {
            if !path.trim().is_empty() {
                return Ok(PathBuf::from(path).join(APP_NAME));
            }
        }
        if let Ok(path) = env::var("HOME") {
            if !path.trim().is_empty() {
                return Ok(PathBuf::from(path)
                    .join(".local")
                    .join("state")
                    .join(APP_NAME));
            }
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "could not determine state directory (LOCALAPPDATA/HOME not set)",
    ))
}
