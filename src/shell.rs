use std::env;
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Copy, Debug)]
pub enum Shell {
    Bash,
    Zsh,
    Pwsh,
}

impl Shell {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "bash" => Some(Self::Bash),
            "zsh" => Some(Self::Zsh),
            "pwsh" | "powershell" => Some(Self::Pwsh),
            _ => None,
        }
    }

    pub fn detect() -> Self {
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

    pub fn history_file_name(self) -> &'static str {
        match self {
            Self::Bash => "history.bash",
            Self::Zsh => "history.zsh",
            Self::Pwsh => "history.pwsh",
        }
    }

    pub fn program(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Pwsh => "pwsh",
        }
    }

    pub fn activation_script(self, context: &str, history_path: &Path) -> String {
        match self {
            Self::Bash => {
                let context_q = sh_single_quote(context);
                let hist_q = sh_single_quote(&posix_display_string(history_path));
                format!(
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
                )
            }
            Self::Zsh => {
                let context_q = sh_single_quote(context);
                let hist_q = sh_single_quote(&posix_display_string(history_path));
                format!(
                    "export CLONCI_CONTEXT={context_q}\n\
export HISTFILE={hist_q}\n\
export HISTSIZE=100000\n\
export SAVEHIST=100000\n\
setopt APPEND_HISTORY INC_APPEND_HISTORY SHARE_HISTORY HIST_IGNORE_DUPS\n\
fc -A\n\
history -c\n\
fc -R \"$HISTFILE\""
                )
            }
            Self::Pwsh => {
                let context_q = ps_single_quote(context);
                let hist_q = ps_single_quote(&display_string(history_path));
                format!(
                    "$env:CLONCI_CONTEXT = {context_q}\n\
Set-PSReadLineOption -HistorySavePath {hist_q}"
                )
            }
        }
    }

    pub fn enter_command(self, context: &str, history_path: &Path) -> Command {
        match self {
            Self::Bash => {
                let mut cmd = Command::new(self.program());
                cmd.arg("-i");
                cmd.env("CLONCI_CONTEXT", context);
                cmd.env("HISTFILE", posix_display_string(history_path));
                cmd.env("HISTSIZE", "100000");
                cmd.env("HISTFILESIZE", "200000");
                cmd.env("PROMPT_COMMAND", "history -a; history -n");
                cmd
            }
            Self::Zsh => {
                let mut cmd = Command::new(self.program());
                cmd.arg("-i");
                cmd.env("CLONCI_CONTEXT", context);
                cmd.env("HISTFILE", posix_display_string(history_path));
                cmd.env("HISTSIZE", "100000");
                cmd.env("SAVEHIST", "100000");
                cmd
            }
            Self::Pwsh => {
                let mut cmd = Command::new(self.program());
                let context_q = ps_single_quote(context);
                let hist_q = ps_single_quote(&display_string(history_path));
                let script = format!(
                    "$env:CLONCI_CONTEXT = {context_q}; Set-PSReadLineOption -HistorySavePath {hist_q}"
                );
                cmd.args(["-NoLogo", "-NoExit", "-Command", &script]);
                cmd
            }
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
