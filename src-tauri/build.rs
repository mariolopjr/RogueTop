use std::process::Command;

fn git_short_hash(repo_path: &str) -> String {
  Command::new("git")
    .args(["-C", repo_path, "rev-parse", "--short", "HEAD"])
    .output()
    .ok()
    .filter(|o| o.status.success())
    .and_then(|o| String::from_utf8(o.stdout).ok())
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "unknown".to_string())
}

fn main() {
  // RogueTop commit hash (always available from the repo itself)
  let roguetop_commit = git_short_hash(".");
  println!("cargo:rustc-env=ROGUETOP_COMMIT={roguetop_commit}");

  // PokeRogue fields: only resolved for offline builds; online builds always show "online"
  let is_offline = std::env::var("CARGO_FEATURE_OFFLINE").is_ok();

  let pokerogue_commit = if is_offline {
    git_short_hash("../src-ext")
  } else {
    "online".to_string()
  };
  println!("cargo:rustc-env=POKEROGUE_COMMIT={pokerogue_commit}");

  let pokerogue_version = if is_offline {
    std::env::var("POKEROGUE_VERSION")
      .or_else(|_| std::env::var("POKEROGUE_BRANCH"))
      .unwrap_or_else(|_| {
        std::fs::read_to_string("../src-ext/package.json")
          .ok()
          .and_then(|s| {
            s.lines()
              .find(|l| l.contains("\"version\""))
              .and_then(|l| l.split('"').nth(3))
              .map(|v| v.to_string())
          })
          .unwrap_or_else(|| "online".to_string())
      })
  } else {
    "online".to_string()
  };
  println!("cargo:rustc-env=POKEROGUE_VERSION={pokerogue_version}");

  tauri_build::build()
}
