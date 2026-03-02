pub fn print_help() {
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
