use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const APP_NAME: &str = "clonci";
const LAST_CONTEXT_FILE: &str = "last_context";

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        print_help();
        return Ok(());
    }

    match args[0].as_str() {
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        "context" => handle_context(&args[1..]),
        "activate" => handle_activate(&args[1..]),
        "enter" => handle_enter(&args[1..]),
        "resume" => handle_resume(&args[1..]),
        "current" => handle_current(&args[1..]),
        other => Err(format!("unknown command '{other}'. Run 'clonci help'.")),
    }
}

fn handle_context(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: clonci context <create|list|delete> ...".to_string());
    }

    match args[0].as_str() {
        "create" => {
            if args.len() != 2 {
                return Err("usage: clonci context create <name>".to_string());
            }
            create_context(&args[1])
        }
        "list" => {
            if args.len() != 1 {
                return Err("usage: clonci context list".to_string());
            }
            list_contexts()
        }
        "delete" => {
            if args.len() != 2 {
                return Err("usage: clonci context delete <name>".to_string());
            }
            delete_context(&args[1])
        }
        other => Err(format!(
            "unknown context subcommand '{other}'. Use create|list|delete."
        )),
    }
}

fn handle_activate(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: clonci activate <context> [--shell bash|zsh|pwsh]".to_string());
    }

    let context = &args[0];
    validate_context_name(context)?;
    ensure_context_exists(context)?;

    let shell = parse_shell_option(&args[1..])?.unwrap_or_else(Shell::detect);
    activate_context(context, shell)
}

fn handle_enter(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: clonci enter <context> [--shell bash|zsh|pwsh]".to_string());
    }

    let context = &args[0];
    validate_context_name(context)?;
    ensure_context_exists(context)?;

    let shell = parse_shell_option(&args[1..])?.unwrap_or_else(Shell::detect);
    enter_context(context, shell)
}

fn handle_resume(args: &[String]) -> Result<(), String> {
    let shell = parse_shell_option(args)?.unwrap_or_else(Shell::detect);
    let context = read_last_context()?
        .ok_or_else(|| "no last context found. Activate or enter a context first.".to_string())?;
    ensure_context_exists(&context)?;
    enter_context(&context, shell)
}

fn activate_context(context: &str, shell: Shell) -> Result<(), String> {
    let history_path = ensure_history_file(context, shell)?;
    write_last_context(context)?;

    let script = shell.activation_script(context, &history_path)?;
    println!("{script}");
    Ok(())
}

fn enter_context(context: &str, shell: Shell) -> Result<(), String> {
    let history_path = ensure_history_file(context, shell)?;
    write_last_context(context)?;

    let mut command = shell.enter_command(context, &history_path)?;
    command.stdin(Stdio::inherit());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let status = command
        .status()
        .map_err(|e| format!("failed to launch shell '{}': {e}", shell.program()))?;

    if let Some(code) = status.code() {
        process::exit(code);
    }

    Ok(())
}

fn handle_current(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("usage: clonci current".to_string());
    }

    if let Ok(active) = env::var("CLONCI_CONTEXT") {
        if !active.trim().is_empty() {
            println!("{active}");
            return Ok(());
        }
    }

    if let Some(last) = read_last_context()? {
        println!("{last}");
        return Ok(());
    }

    println!("(none)");
    Ok(())
}

fn create_context(name: &str) -> Result<(), String> {
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

    println!("created context '{name}'");
    Ok(())
}

fn list_contexts() -> Result<(), String> {
    ensure_state_dirs().map_err(|e| format!("failed to prepare state directory: {e}"))?;

    let dir = contexts_dir().map_err(|e| e.to_string())?;
    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("failed to read contexts directory '{}': {e}", dir.display()))?;

    let current = env::var("CLONCI_CONTEXT")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let last = read_last_context()?;

    let mut contexts: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();

    contexts.sort();

    if contexts.is_empty() {
        println!("(no contexts)");
        return Ok(());
    }

    for name in contexts {
        let mut markers = Vec::new();
        if current.as_deref() == Some(name.as_str()) {
            markers.push("active");
        }
        if last.as_deref() == Some(name.as_str()) {
            markers.push("last");
        }

        if markers.is_empty() {
            println!("{name}");
        } else {
            println!("{name}\t[{}]", markers.join(", "));
        }
    }

    Ok(())
}

fn delete_context(name: &str) -> Result<(), String> {
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

    println!("deleted context '{name}'");
    Ok(())
}

fn ensure_context_exists(name: &str) -> Result<(), String> {
    let path = context_dir(name).map_err(|e| e.to_string())?;
    if path.exists() {
        return Ok(());
    }

    Err(format!(
        "context '{name}' does not exist. Create it with: clonci context create {name}"
    ))
}

fn parse_shell_option(args: &[String]) -> Result<Option<Shell>, String> {
    if args.is_empty() {
        return Ok(None);
    }

    if args.len() != 2 || args[0] != "--shell" {
        return Err("expected optional flag: --shell <bash|zsh|pwsh>".to_string());
    }

    Shell::from_str(&args[1])
        .map(Some)
        .ok_or_else(|| format!("unsupported shell '{}'. Use bash, zsh, or pwsh.", args[1]))
}

fn validate_context_name(name: &str) -> Result<(), String> {
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

fn ensure_history_file(context: &str, shell: Shell) -> Result<PathBuf, String> {
    let dir = context_dir(context).map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| {
        format!(
            "failed to ensure context directory '{}': {e}",
            dir.display()
        )
    })?;

    let path = dir.join(shell.history_file_name());
    if !path.exists() {
        fs::write(&path, b"")
            .map_err(|e| format!("failed to create history file '{}': {e}", path.display()))?;
    }

    Ok(path)
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

fn write_last_context(name: &str) -> Result<(), String> {
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

fn read_last_context() -> Result<Option<String>, String> {
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

#[derive(Clone, Copy, Debug)]
enum Shell {
    Bash,
    Zsh,
    Pwsh,
}

impl Shell {
    fn from_str(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "bash" => Some(Self::Bash),
            "zsh" => Some(Self::Zsh),
            "pwsh" | "powershell" => Some(Self::Pwsh),
            _ => None,
        }
    }

    fn detect() -> Self {
        if let Ok(shell) = env::var("SHELL") {
            if let Some(detected) = Self::from_shell_path(&shell) {
                return detected;
            }
        }

        if cfg!(windows) {
            Self::Pwsh
        } else {
            Self::Bash
        }
    }

    fn from_shell_path(path: &str) -> Option<Self> {
        let lowered = path.to_ascii_lowercase();
        if lowered.contains("bash") {
            Some(Self::Bash)
        } else if lowered.contains("zsh") {
            Some(Self::Zsh)
        } else if lowered.contains("pwsh") || lowered.contains("powershell") {
            Some(Self::Pwsh)
        } else {
            None
        }
    }

    fn history_file_name(self) -> &'static str {
        match self {
            Self::Bash => "history.bash",
            Self::Zsh => "history.zsh",
            Self::Pwsh => "history.pwsh",
        }
    }

    fn program(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Pwsh => "pwsh",
        }
    }

    fn activation_script(self, context: &str, history_path: &Path) -> Result<String, String> {
        match self {
            Self::Bash => {
                let context_q = sh_single_quote(context);
                let hist_q = sh_single_quote(&posix_display_string(history_path));
                Ok(format!(
                    "export CLONCI_CONTEXT={context_q}\n\
export HISTFILE={hist_q}\n\
export HISTSIZE=100000\n\
export HISTFILESIZE=200000\n\
shopt -s histappend\n\
if [[ \"${{PROMPT_COMMAND:-}}\" != *\"history -a; history -n\"* ]]; then\n\
  if [[ -n \"${{PROMPT_COMMAND:-}}\" ]]; then\n\
    PROMPT_COMMAND=\"history -a; history -n; ${{PROMPT_COMMAND}}\"\n\
  else\n\
    PROMPT_COMMAND=\"history -a; history -n\"\n\
  fi\n\
fi\n\
history -a\n\
history -c\n\
history -r \"$HISTFILE\""
                ))
            }
            Self::Zsh => {
                let context_q = sh_single_quote(context);
                let hist_q = sh_single_quote(&posix_display_string(history_path));
                Ok(format!(
                    "export CLONCI_CONTEXT={context_q}\n\
export HISTFILE={hist_q}\n\
export HISTSIZE=100000\n\
export SAVEHIST=100000\n\
setopt APPEND_HISTORY INC_APPEND_HISTORY SHARE_HISTORY HIST_IGNORE_DUPS\n\
fc -A\n\
history -c\n\
fc -R \"$HISTFILE\""
                ))
            }
            Self::Pwsh => {
                let context_q = ps_single_quote(context);
                let hist_q = ps_single_quote(&display_string(history_path));
                Ok(format!(
                    "$env:CLONCI_CONTEXT = {context_q}\n\
Set-PSReadLineOption -HistorySavePath {hist_q}"
                ))
            }
        }
    }

    fn enter_command(self, context: &str, history_path: &Path) -> Result<Command, String> {
        match self {
            Self::Bash => {
                let mut cmd = Command::new(self.program());
                cmd.arg("-i");
                cmd.env("CLONCI_CONTEXT", context);
                cmd.env("HISTFILE", posix_display_string(history_path));
                cmd.env("HISTSIZE", "100000");
                cmd.env("HISTFILESIZE", "200000");
                cmd.env("PROMPT_COMMAND", "history -a; history -n");
                Ok(cmd)
            }
            Self::Zsh => {
                let mut cmd = Command::new(self.program());
                cmd.arg("-i");
                cmd.env("CLONCI_CONTEXT", context);
                cmd.env("HISTFILE", posix_display_string(history_path));
                cmd.env("HISTSIZE", "100000");
                cmd.env("SAVEHIST", "100000");
                Ok(cmd)
            }
            Self::Pwsh => {
                let mut cmd = Command::new(self.program());
                let context_q = ps_single_quote(context);
                let hist_q = ps_single_quote(&display_string(history_path));
                let script = format!(
                    "$env:CLONCI_CONTEXT = {context_q}; Set-PSReadLineOption -HistorySavePath {hist_q}"
                );
                cmd.args(["-NoLogo", "-NoExit", "-Command", &script]);
                Ok(cmd)
            }
        }
    }
}

fn sh_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn ps_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn display_string(path: &Path) -> String {
    path_to_string(path.as_os_str())
}

fn posix_display_string(path: &Path) -> String {
    let raw = display_string(path);
    if cfg!(windows) {
        raw.replace('\\', "/")
    } else {
        raw
    }
}

fn path_to_string(value: &OsStr) -> String {
    value.to_string_lossy().to_string()
}

fn print_help() {
    println!(
        "clonci - context-based shell history manager\n\n\
USAGE:\n\
  clonci context create <name>\n\
  clonci context list\n\
  clonci context delete <name>\n\
  clonci activate <name> [--shell bash|zsh|pwsh]\n\
  clonci enter <name> [--shell bash|zsh|pwsh]\n\
  clonci resume [--shell bash|zsh|pwsh]\n\
  clonci current\n\n\
EXAMPLES:\n\
  clonci context create work\n\
  eval \"$(clonci activate work --shell bash)\"\n\
  clonci enter personal --shell pwsh\n\
  clonci resume --shell bash\n\
  clonci context list\n\n\
NOTES:\n\
  - Context names allow only letters, numbers, '-' and '_'.\n\
  - History is stored per-context under your local state directory.\n\
  - Use 'activate' to switch in the current shell, and 'enter' to open a new context-bound shell.\n\
  - 'resume' opens the most recently activated/entered context."
    );
}
