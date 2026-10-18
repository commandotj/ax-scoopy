use std::{
    io::IsTerminal,
    path::{Path, PathBuf},
};

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{OutputFormat, atomic, resolve_format, write_json};

const BUNDLED_SKILL: &str = include_str!("../skill/cap/SKILL.md");

// The cap-demo companion skill ships as a directory tree (scripts, cursor and
// music assets) embedded into the binary alongside the main cap skill.
static CAP_DEMO_SKILL: include_dir::Dir<'static> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/skill/cap-demo");

// Flatten every embedded file (recursively) so each becomes its own install
// change. include_dir gives direct files via `files()` and nested dirs via
// `dirs()`; file paths are already relative to the embedded root.
fn collect_demo_files<'a>(dir: &'a include_dir::Dir<'a>, out: &mut Vec<&'a include_dir::File<'a>>) {
    for file in dir.files() {
        out.push(file);
    }
    for sub in dir.dirs() {
        collect_demo_files(sub, out);
    }
}

#[derive(Args)]
pub struct AgentsArgs {
    #[command(subcommand)]
    command: AgentsCommands,
}

#[derive(Subcommand)]
enum AgentsCommands {
    Install(InstallArgs),
}

#[derive(Clone, Copy, ValueEnum)]
enum AgentTarget {
    Codex,
    Claude,
    Cursor,
}

#[derive(Args)]
struct InstallArgs {
    #[arg(long, value_enum)]
    target: AgentTarget,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    yes: bool,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlannedChange {
    component: &'static str,
    path: PathBuf,
    action: &'static str,
    value: Value,
    #[serde(skip)]
    expected: Option<Vec<u8>>,
    #[serde(skip)]
    content: Option<Vec<u8>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallResult {
    target: &'static str,
    dry_run: bool,
    applied: bool,
    changes: Vec<PlannedChange>,
}

fn home_dir() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "Could not locate the home directory".to_string())
}

fn target_name(target: AgentTarget) -> &'static str {
    match target {
        AgentTarget::Codex => "codex",
        AgentTarget::Claude => "claude",
        AgentTarget::Cursor => "cursor",
    }
}

fn skill_path(target: AgentTarget) -> Result<PathBuf, String> {
    let home = home_dir()?;
    Ok(match target {
        AgentTarget::Codex => std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"))
            .join("skills/cap/SKILL.md"),
        AgentTarget::Claude => home.join(".claude/skills/cap/SKILL.md"),
        AgentTarget::Cursor => home.join(".cursor/skills/cap/SKILL.md"),
    })
}

// The cap-demo skill installs as a directory sibling of the cap skill.
fn cap_demo_skill_dir(target: AgentTarget) -> Result<PathBuf, String> {
    let home = home_dir()?;
    Ok(match target {
        AgentTarget::Codex => std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"))
            .join("skills/cap-demo"),
        AgentTarget::Claude => home.join(".claude/skills/cap-demo"),
        AgentTarget::Cursor => home.join(".cursor/skills/cap-demo"),
    })
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Install path has no parent directory".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    std::fs::write(&temporary, content).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::metadata(path)
            .map(|metadata| metadata.permissions())
            .unwrap_or_else(|_| std::fs::Permissions::from_mode(0o644));
        std::fs::set_permissions(&temporary, permissions).map_err(|error| error.to_string())?;
    }
    if let Err(error) = atomic::replace(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn print_changes(changes: &[PlannedChange]) -> Result<(), String> {
    for change in changes {
        println!(
            "{} {}: {}",
            change.action,
            change.component,
            change.path.display()
        );
        println!(
            "  {}",
            serde_json::to_string(&change.value).map_err(|error| error.to_string())?
        );
    }
    Ok(())
}

fn confirm() -> Result<(), String> {
    if !std::io::stdin().is_terminal() {
        return Err("Non-interactive installation requires --yes".to_string());
    }
    eprint!("Apply these changes? [y/N] ");
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|error| error.to_string())?;
    if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        return Err("Installation cancelled".to_string());
    }
    Ok(())
}

impl InstallArgs {
    fn plan(&self) -> Result<Vec<PlannedChange>, String> {
        let mut changes = Vec::new();
        {
            let path = skill_path(self.target)?;
            let expected = read_optional(&path)?;
            let action = if expected.as_deref() == Some(BUNDLED_SKILL.as_bytes()) {
                "unchanged"
            } else if expected.is_some() {
                "replace"
            } else {
                "create"
            };
            let content = (action != "unchanged").then(|| BUNDLED_SKILL.as_bytes().to_vec());
            changes.push(PlannedChange {
                component: "skill",
                path,
                action,
                value: json!({ "name": "cap", "content": BUNDLED_SKILL }),
                expected,
                content,
            });

            // Ship the cap-demo companion skill as one change per embedded file.
            let demo_dir = cap_demo_skill_dir(self.target)?;
            let mut demo_files = Vec::new();
            collect_demo_files(&CAP_DEMO_SKILL, &mut demo_files);
            demo_files.sort_by(|a, b| a.path().cmp(b.path()));
            for file in demo_files {
                let relative = file.path();
                let dest = demo_dir.join(relative);
                let expected = read_optional(&dest)?;
                let bytes = file.contents();
                let action = if expected.as_deref() == Some(bytes) {
                    "unchanged"
                } else if expected.is_some() {
                    "replace"
                } else {
                    "create"
                };
                let content = (action != "unchanged").then(|| bytes.to_vec());
                changes.push(PlannedChange {
                    component: "skill",
                    path: dest,
                    action,
                    value: json!({
                        "name": "cap-demo",
                        "file": relative.to_string_lossy(),
                    }),
                    expected,
                    content,
                });
            }
        }
        Ok(changes)
    }

    fn apply(&self, changes: &[PlannedChange]) -> Result<(), String> {
        for change in changes {
            if read_optional(&change.path)? != change.expected {
                return Err(format!(
                    "{} changed after the installation preview; review a new preview before applying",
                    change.path.display()
                ));
            }
        }
        for change in changes {
            if let Some(content) = &change.content {
                atomic_write(&change.path, content)?;
            }
        }
        Ok(())
    }
}

impl AgentsArgs {
    pub fn run(self, global_json: bool) -> Result<(), String> {
        let format = match &self.command {
            AgentsCommands::Install(args) => resolve_format(global_json, args.format),
        };
        let result = self.run_inner(global_json);
        if let Err(error) = &result
            && format == OutputFormat::Json
        {
            let _ = write_json(&json!({ "error": error }));
        }
        result
    }

    fn run_inner(self, global_json: bool) -> Result<(), String> {
        match self.command {
            AgentsCommands::Install(args) => {
                let format = resolve_format(global_json, args.format);
                let changes = args.plan()?;
                if args.dry_run {
                    let result = InstallResult {
                        target: target_name(args.target),
                        dry_run: true,
                        applied: false,
                        changes,
                    };
                    return match format {
                        OutputFormat::Json => write_json(&result),
                        OutputFormat::Text => print_changes(&result.changes),
                    };
                }
                if format == OutputFormat::Text {
                    print_changes(&changes)?;
                } else if !args.yes {
                    eprintln!(
                        "{}",
                        serde_json::to_string_pretty(&changes).map_err(|error| error.to_string())?
                    );
                }
                if !args.yes {
                    confirm()?;
                }
                args.apply(&changes)?;
                let result = InstallResult {
                    target: target_name(args.target),
                    dry_run: false,
                    applied: true,
                    changes,
                };
                match format {
                    OutputFormat::Json => write_json(&result),
                    OutputFormat::Text => {
                        println!("Installed Scoopy agent components for {}.", result.target);
                        Ok(())
                    }
                }
            }
        }
    }
}
