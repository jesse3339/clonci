use std::env;
use std::process::{self, Stdio};

use crate::help;
use crate::shell::Shell;
use crate::state;

pub fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    dispatch(&args)
}

fn dispatch(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        help::print_help();
        return Ok(());
    }

    match args[0].as_str() {
        "help" | "-h" | "--help" => {
            help::print_help();
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
            state::create_context(&args[1])?;
            println!("created context '{}'", args[1]);
            Ok(())
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
            state::delete_context(&args[1])?;
            println!("deleted context '{}'", args[1]);
            Ok(())
        }
        other => Err(format!(
            "unknown context subcommand '{other}'. Use create|list|delete."
        )),
    }
}

fn list_contexts() -> Result<(), String> {
    let current = env::var("CLONCI_CONTEXT")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let last = state::read_last_context()?;
    let contexts = state::list_context_names()?;

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

fn handle_activate(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: clonci activate <context> [--shell bash|zsh|pwsh]".to_string());
    }

    let context = &args[0];
    state::validate_context_name(context)?;
    state::ensure_context_exists(context)?;

    let shell = parse_shell_option(&args[1..])?.unwrap_or_else(Shell::detect);
    activate_context(context, shell)
}

fn handle_enter(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: clonci enter <context> [--shell bash|zsh|pwsh]".to_string());
    }

    let context = &args[0];
    state::validate_context_name(context)?;
    state::ensure_context_exists(context)?;

    let shell = parse_shell_option(&args[1..])?.unwrap_or_else(Shell::detect);
    enter_context(context, shell)
}

fn handle_resume(args: &[String]) -> Result<(), String> {
    let shell = parse_shell_option(args)?.unwrap_or_else(Shell::detect);
    let context = state::read_last_context()?
        .ok_or_else(|| "no last context found. Activate or enter a context first.".to_string())?;
    state::ensure_context_exists(&context)?;
    enter_context(&context, shell)
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

    if let Some(last) = state::read_last_context()? {
        println!("{last}");
        return Ok(());
    }

    println!("(none)");
    Ok(())
}

fn activate_context(context: &str, shell: Shell) -> Result<(), String> {
    let history_path = state::ensure_history_file(context, shell.history_file_name())?;
    state::write_last_context(context)?;

    let script = shell.activation_script(context, &history_path);
    println!("{script}");
    Ok(())
}

fn enter_context(context: &str, shell: Shell) -> Result<(), String> {
    let history_path = state::ensure_history_file(context, shell.history_file_name())?;
    state::write_last_context(context)?;

    let mut command = shell.enter_command(context, &history_path);
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
