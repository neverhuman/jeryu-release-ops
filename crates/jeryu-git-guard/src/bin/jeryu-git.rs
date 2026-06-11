//! `jeryu-git` — the confined-agent git wrapper.
//!
//! Installed as the ONLY `git` on a sandboxed agent's `PATH`. It consults the pure
//! [`jeryu_git_guard::git_command_decision`] verdict and then either `exec`s the real
//! git (replacing this process, so no overhead) or prints typed repair guidance and
//! exits non-zero. The agent's assigned branch comes from `JERYU_BRANCH`; the real
//! git binary path comes from `JERYU_REAL_GIT` (default `/usr/bin/git`).

use std::os::unix::process::CommandExt;
use std::process::Command;

use jeryu_git_guard::{DENY_EXIT_CODE, GitDecision, git_command_decision};

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let assigned_branch = match std::env::var("JERYU_BRANCH") {
        Ok(branch) if !branch.trim().is_empty() => branch,
        _ => {
            eprintln!("jeryu-git: missing required JERYU_BRANCH");
            std::process::exit(2);
        }
    };

    match git_command_decision(&argv, &assigned_branch) {
        GitDecision::Allow => {
            let real_git = match std::env::var("JERYU_REAL_GIT") {
                Ok(path) if !path.trim().is_empty() => path,
                _ => {
                    eprintln!("jeryu-git: missing required JERYU_REAL_GIT");
                    std::process::exit(2);
                }
            };
            // `exec` replaces this process; it only returns on failure.
            let err = Command::new(&real_git).args(&argv).exec();
            eprintln!("jeryu-git: failed to exec real git ({real_git}): {err}");
            std::process::exit(127);
        }
        GitDecision::Deny(error) => {
            eprintln!("{}", error.render());
            std::process::exit(DENY_EXIT_CODE);
        }
    }
}
