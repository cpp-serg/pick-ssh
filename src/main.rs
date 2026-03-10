mod app;
mod event;
mod history;
mod ssh_config;
mod tree;
mod ui;

use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use app::App;
use ssh_config::load_ssh_config;

#[derive(Parser)]
#[command(name = "pick-ssh", about = "fzf-like SSH host selector")]
struct Cli {
    /// Completion mode: print selected hostname to stdout instead of running ssh
    #[arg(long)]
    complete: bool,

    /// Pre-fill the filter field with this value
    #[arg(long)]
    filter: Option<String>,

    /// Current command line buffer (for shell integration). Supersedes --filter and --cmd.
    #[arg(long)]
    line: Option<String>,

    /// Print shell integration snippet and exit. Supported shells: zsh, bash, fish
    #[arg(long, value_name = "SHELL")]
    embed: Option<Option<String>>,
}

/// Context derived from --line or legacy --filter/--cmd flags.
struct CompletionCtx {
    filter: Option<String>,
    /// Text to prepend before the hostname in output
    output_prefix: String,
    /// Text to append after the hostname in output (e.g. ":" for scp/rsync)
    output_suffix: &'static str,
}

fn parse_line(line: &str) -> CompletionCtx {
    if line.is_empty() {
        return CompletionCtx {
            filter: None,
            output_prefix: "ssh ".to_string(),
            output_suffix: "",
        };
    }

    let cmd = line.split_whitespace().next().unwrap_or("");
    let suffix = match cmd {
        "scp" | "rsync" => ":",
        _ => "",
    };

    if line.ends_with(' ') {
        CompletionCtx {
            filter: None,
            output_prefix: line.to_string(),
            output_suffix: suffix,
        }
    } else {
        match line.rfind(' ') {
            Some(pos) => CompletionCtx {
                filter: Some(line[pos + 1..].to_string()),
                output_prefix: line[..pos + 1].to_string(),
                output_suffix: suffix,
            },
            // Single word, no spaces — treat like empty line (run ssh) but with filter
            None => CompletionCtx {
                filter: Some(line.to_string()),
                output_prefix: "ssh ".to_string(),
                output_suffix: "",
            },
        }
    }
}

fn print_embed(shell: &str) -> Result<()> {
    let bin = std::env::current_exe()?.to_string_lossy().into_owned();

    match shell {
        "zsh" => {
            println!(
                r#"# pick-ssh: TUI-assisted SSH host completion
# Add this to your ~/.zshrc

_pick-ssh_complete() {{
  local line="$LBUFFER"
  local result
  result="$("{bin}" --complete --line "$line")"
  if [[ -z "$result" ]]; then
    zle reset-prompt
    return
  fi
  if [[ "$result" == print:* ]]; then
    BUFFER="${{result#print:}}"
    CURSOR=$#BUFFER
    zle reset-prompt
    return
  fi
  if [[ -z "$line" || "$line" != *" "* ]]; then
    BUFFER="$result"
    zle accept-line
  else
    LBUFFER="$result"
    zle reset-prompt
  fi
}}
zle -N _pick-ssh_complete
bindkey '^s' _pick-ssh_complete"#
            );
        }
        "bash" => {
            println!(
                r#"# pick-ssh: TUI-assisted SSH host completion
# Add this to your ~/.bashrc

_pick-ssh_complete() {{
  local line="${{READLINE_LINE:0:$READLINE_POINT}}"
  local result
  result="$("{bin}" --complete --line "$line")"
  if [[ -n "$result" ]]; then
    if [[ "$result" == print:* ]]; then
      READLINE_LINE="${{result#print:}}"
      READLINE_POINT=${{#READLINE_LINE}}
    elif [[ -z "$line" || "$line" != *" "* ]]; then
      READLINE_LINE="$result"
      READLINE_POINT=${{#READLINE_LINE}}
    else
      READLINE_LINE="${{result}}${{READLINE_LINE:$READLINE_POINT}}"
      READLINE_POINT=${{#result}}
    fi
  fi
}}
bind -x '"\C-s": _pick-ssh_complete'"#
            );
        }
        "fish" => {
            println!(
                r#"# pick-ssh: TUI-assisted SSH host completion
# Add this to your ~/.config/fish/config.fish

function _pick-ssh_complete
  set -l line (commandline --cut-at-cursor)
  set -l result ("{bin}" --complete --line "$line")
  if test -z "$result"
    return
  end
  if string match -q 'print:*' -- "$result"
    set result (string replace -r '^print:' '' -- "$result")
    commandline --replace -- "$result"
    commandline -C (string length -- "$result")
    return
  end
  if test -z "$line" -o (not string match -q '* *' -- "$line")
    commandline "$result"
    commandline -f execute
  else
    set -l rest (commandline | string sub -s (math (string length -- "$line") + 1))
    commandline --replace -- "$result$rest"
    commandline -C (string length -- "$result")
  end
end
bind \cs _pick-ssh_complete"#
            );
        }
        _ => {
            eprintln!("unsupported shell: {shell} (supported: zsh, bash, fish)");
            std::process::exit(1);
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // --embed: print shell integration snippet and exit
    if let Some(shell_opt) = &cli.embed {
        let shell = shell_opt.as_deref().unwrap_or("zsh");
        return print_embed(shell);
    }

    // Derive completion context: --line supersedes --filter
    let ctx = if let Some(ref line) = cli.line {
        parse_line(line)
    } else {
        CompletionCtx {
            filter: cli.filter.clone(),
            output_prefix: String::new(),
            output_suffix: "",
        }
    };

    let root = load_ssh_config()?;
    let hist = history::load_history();
    let mut app = App::new(root, hist);

    // Apply initial filter
    if let Some(ref f) = ctx.filter {
        app.filter.clone_from(f);
        app.update_filter();
        app.snap_to_first_host();

        // Single result → auto-select immediately
        if app.visible_host_count() == 1 {
            app.select();
        }
    } else {
        app.snap_to_first_host();
    }

    // Skip TUI if already auto-selected
    if !app.should_quit {
        // Setup terminal — use stderr so stdout stays clean for --complete mode
        enable_raw_mode()?;
        let mut stderr = io::stderr();
        execute!(stderr, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stderr);
        let mut terminal = Terminal::new(backend)?;

        // Main loop
        while !app.should_quit {
            terminal.draw(|f| ui::draw(f, &mut app))?;
            event::handle_events(&mut app)?;
        }

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    }

    // Act on selection
    if let Some(host) = app.chosen_host {
        // Build the command that would be executed
        let full = app
            .chosen_full_cmd
            .as_deref()
            .map(String::from)
            .unwrap_or_else(|| format!("ssh {host}"));

        if cli.complete {
            let output = if app.print_only {
                // Tab in completion mode: output the full command
                full.clone()
            } else if app.chosen_full_cmd.is_some() && ctx.output_prefix == "ssh " {
                // Enter in ssh-launch mode: prefer the full history command
                full.clone()
            } else {
                format!("{}{host}{}", ctx.output_prefix, ctx.output_suffix)
            };
            if app.print_only {
                print!("print:{output}");
            } else {
                print!("{output}");
            }
        } else if app.print_only {
            // Ctrl+Enter in standalone mode: print command, don't exec
            println!("{full}");
        } else {
            // Enter in standalone mode: exec the command
            eprintln!("{full}");
            let parts: Vec<&str> = full.split_whitespace().collect();
            let err = Command::new(parts[0]).args(&parts[1..]).exec();
            return Err(err.into());
        }
    }

    Ok(())
}
