use zed_extension_api::{
    self as zed,
    SlashCommand, SlashCommandArgumentCompletion, SlashCommandOutput, SlashCommandOutputSection,
    Worktree,
};

struct VibeCodeGuardian;

// ── Git helpers ───────────────────────────────────────────────────────────────

fn git_run(worktree: &Worktree, args: &[&str]) -> Result<String, String> {
    let git = worktree
        .which("git")
        .ok_or_else(|| "git not found in PATH".to_string())?;
    let output = std::process::Command::new(&git)
        .args(args)
        .current_dir(worktree.root_path())
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn make_output(text: impl Into<String>) -> SlashCommandOutput {
    let text: String = text.into();
    let len = text.len();
    SlashCommandOutput {
        sections: vec![SlashCommandOutputSection {
            range: (0..len).into(),
            label: "Vibe Code Guardian".to_string(),
        }],
        text,
    }
}

// ── Slash command logic ───────────────────────────────────────────────────────

fn cmd_save(args: &[String], worktree: Option<&Worktree>) -> Result<SlashCommandOutput, String> {
    let wt = worktree.ok_or("No workspace open")?;
    let name = if args.is_empty() || args[0].trim().is_empty() {
        "vibe-checkpoint".to_string()
    } else {
        args[0].trim().to_string()
    };

    // Ensure we are in a git repo; init if not
    if git_run(wt, &["rev-parse", "--git-dir"]).is_err() {
        git_run(wt, &["init"])?;
        git_run(wt, &["commit", "--allow-empty", "-m", "vibe: initial"])?;
    }

    // Stage all changes
    git_run(wt, &["add", "-A"])?;

    // Check if there is anything to commit
    let status = git_run(wt, &["status", "--porcelain"])?;
    if status.is_empty() {
        return Ok(make_output(
            "✓ Nothing to save – working tree is clean.",
        ));
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let msg = format!("vibe: {name} [{timestamp}]");
    git_run(wt, &["commit", "-m", &msg])?;

    let hash = git_run(wt, &["rev-parse", "--short", "HEAD"])?;
    Ok(make_output(format!(
        "✓ Checkpoint saved: \"{name}\" ({hash})\n\nChanged files:\n{status}"
    )))
}

fn cmd_timeline(worktree: Option<&Worktree>) -> Result<SlashCommandOutput, String> {
    let wt = worktree.ok_or("No workspace open")?;
    let log = git_run(
        wt,
        &[
            "log",
            "--oneline",
            "--decorate",
            "--all",
            "--max-count=30",
            "--grep=vibe:",
        ],
    )
    .unwrap_or_default();

    if log.is_empty() {
        return Ok(make_output(
            "No vibe checkpoints found.\n\nUse /vibe-save to create your first checkpoint.",
        ));
    }

    Ok(make_output(format!(
        "Vibe Code Guardian – Checkpoint Timeline (last 30)\n{sep}\n{log}\n\nUse /vibe-rollback <hash> to restore a checkpoint.",
        sep = "─".repeat(60),
    )))
}

fn cmd_diff(worktree: Option<&Worktree>) -> Result<SlashCommandOutput, String> {
    let wt = worktree.ok_or("No workspace open")?;
    let staged = git_run(wt, &["diff", "--stat", "--cached"]).unwrap_or_default();
    let unstaged = git_run(wt, &["diff", "--stat"]).unwrap_or_default();
    let untracked = git_run(wt, &["ls-files", "--others", "--exclude-standard"])
        .map(|s| {
            if s.is_empty() {
                String::new()
            } else {
                format!("\nUntracked files:\n{s}")
            }
        })
        .unwrap_or_default();

    if staged.is_empty() && unstaged.is_empty() && untracked.is_empty() {
        return Ok(make_output("✓ Working tree is clean – no changes."));
    }

    let mut out = String::from("Vibe Code Guardian – Current Changes\n");
    out.push_str(&"─".repeat(60));
    out.push('\n');
    if !staged.is_empty() {
        out.push_str("Staged:\n");
        out.push_str(&staged);
        out.push('\n');
    }
    if !unstaged.is_empty() {
        out.push_str("Unstaged:\n");
        out.push_str(&unstaged);
        out.push('\n');
    }
    out.push_str(&untracked);
    Ok(make_output(out))
}

fn cmd_rollback(args: &[String], worktree: Option<&Worktree>) -> Result<SlashCommandOutput, String> {
    let wt = worktree.ok_or("No workspace open")?;

    if args.is_empty() || args[0].trim().is_empty() {
        // No arg → show last 10 vibe checkpoints
        let log = git_run(
            wt,
            &[
                "log",
                "--oneline",
                "--decorate",
                "--max-count=10",
                "--grep=vibe:",
            ],
        )
        .unwrap_or_default();
        if log.is_empty() {
            return Ok(make_output(
                "No vibe checkpoints found to rollback to.",
            ));
        }
        return Ok(make_output(format!(
            "Available checkpoints (recent 10):\n{log}\n\nRun /vibe-rollback <hash> to restore."
        )));
    }

    let hash = args[0].trim();
    // Validate the hash exists
    let full = git_run(wt, &["rev-parse", "--verify", hash])
        .map_err(|_| format!("Commit '{hash}' not found."))?;
    let short = &full[..8.min(full.len())];

    // Hard reset to the target commit
    git_run(wt, &["reset", "--hard", hash])?;

    Ok(make_output(format!(
        "✓ Rolled back to checkpoint {short}\n\nWorking tree restored. Use /vibe-save to create a new checkpoint from here.",
    )))
}

fn cmd_checkpoint(args: &[String], worktree: Option<&Worktree>) -> Result<SlashCommandOutput, String> {
    // /vibe-checkpoint is an alias for /vibe-save with a required name
    if args.is_empty() || args[0].trim().is_empty() {
        return Ok(make_output(
            "Usage: /vibe-checkpoint <name> [description]\n\nExample: /vibe-checkpoint 'auth feature working'\n\nCreates a named git checkpoint (commit) of your current state.",
        ));
    }
    cmd_save(args, worktree)
}

// ── Extension impl ────────────────────────────────────────────────────────────

impl zed::Extension for VibeCodeGuardian {
    fn new() -> Self {
        VibeCodeGuardian
    }

    fn complete_slash_command_argument(
        &self,
        command: SlashCommand,
        _args: Vec<String>,
    ) -> Result<Vec<SlashCommandArgumentCompletion>, String> {
        match command.name.as_str() {
            "vibe-save" | "vibe-checkpoint" => Ok(vec![
                SlashCommandArgumentCompletion {
                    label: "my-feature-working".to_string(),
                    new_text: "my-feature-working".to_string(),
                    run_command: false,
                },
                SlashCommandArgumentCompletion {
                    label: "before-refactor".to_string(),
                    new_text: "before-refactor".to_string(),
                    run_command: false,
                },
            ]),
            _ => Ok(Vec::new()),
        }
    }

    fn run_slash_command(
        &self,
        command: SlashCommand,
        args: Vec<String>,
        worktree: Option<&Worktree>,
    ) -> Result<SlashCommandOutput, String> {
        match command.name.as_str() {
            "vibe-save"        => cmd_save(&args, worktree),
            "vibe-checkpoint"  => cmd_checkpoint(&args, worktree),
            "vibe-timeline"    => cmd_timeline(worktree),
            "vibe-diff"        => cmd_diff(worktree),
            "vibe-rollback"    => cmd_rollback(&args, worktree),
            other => Err(format!("Unknown command: {other}")),
        }
    }
}

zed::register_extension!(VibeCodeGuardian);
