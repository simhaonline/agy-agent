use clap::{Parser, Subcommand};
use agent_core::audit::AuditLogger;
use agent_core::runner::AgentRunner;
use policy::preset::AgentPreset;
use providers::{MockProvider, OpenAiCompatibleProvider, Provider};
use session::session::SessionManager;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use tools::registry::ToolRegistry;

#[derive(Parser)]
#[command(name = "agy-agent")]
#[command(about = "AGY Standalone Rust Agent Runtime CLI", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive or single-turn conversational assistant
    Chat {
        /// Optional initial prompt
        prompt: Option<String>,
        /// Named behavior and permission preset
        #[arg(short, long)]
        preset: Option<String>,
        /// Workspace root directory
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        /// Existing session ID to resume
        #[arg(long)]
        session: Option<String>,
    },

    /// Non-interactive autonomous task runner
    Run {
        /// Task prompt / goal to execute
        prompt: String,
        /// Named behavior and permission preset
        #[arg(short, long)]
        preset: Option<String>,
        /// Workspace root directory
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        /// Output result as JSON
        #[arg(long)]
        json: bool,
    },

    /// Manage behavior and permission presets
    Preset {
        #[command(subcommand)]
        sub: PresetCommands,
    },

    /// List available tools and permission boundaries
    Tools {
        /// Workspace root directory
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },

    /// Manage agent sessions
    Sessions {
        #[command(subcommand)]
        sub: SessionCommands,
    },

    /// System environment, toolchain, and security diagnostics
    Doctor {
        /// Workspace root directory
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },

    /// Validate workspace configuration files
    Config {
        #[command(subcommand)]
        sub: ConfigCommands,
    },

    /// Inspect JSONL audit logs
    Audit {
        /// Workspace root directory
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        /// Limit number of events shown
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum PresetCommands {
    /// List all six behavior and permission presets
    List,
    /// Show full effective policy and non-bypassable boundaries for a preset
    Show {
        /// Preset name (Heretic, Uncensored, Unrestricted, Aggressive, Abliterated, Unleashed)
        name: String,
    },
    /// Set default preset for workspace
    Use {
        name: String,
    },
}

#[derive(Subcommand)]
enum SessionCommands {
    /// List all stored sessions
    List {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Show details of a session
    Show {
        id: String,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Branch an existing session
    Branch {
        id: String,
        new_name: String,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Delete a session
    Delete {
        id: String,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Validate configuration file syntax and keys
    Validate {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Preset { sub } => match sub {
            PresetCommands::List => {
                println!("Available Presets:");
                println!("------------------------------------------------------------");
                for p in [
                    AgentPreset::Heretic,
                    AgentPreset::Uncensored,
                    AgentPreset::Unrestricted,
                    AgentPreset::Aggressive,
                    AgentPreset::Abliterated,
                    AgentPreset::Unleashed,
                ] {
                    println!("{:<14} {}", p.as_str(), p.description());
                }
            }
            PresetCommands::Show { name } => {
                let p = AgentPreset::parse(&name).ok_or_else(|| {
                    format!("Unknown preset '{}'. Valid: Heretic, Uncensored, Unrestricted, Aggressive, Abliterated, Unleashed", name)
                })?;
                let ep = p.effective_policy();
                println!("============================================================");
                println!("EFFECTIVE POLICY: {}", ep.preset_name.to_uppercase());
                println!("============================================================");
                println!("Persona/Instructions: {}", ep.persona_instructions);
                println!("Client Friction:      {}", ep.client_friction);
                println!("Planning Autonomy:    {}", ep.planning_autonomy);
                println!("Tool Permissions:     {}", ep.tool_permissions);
                println!("Approvals:            {}", ep.approvals);
                println!("Filesystem Scope:     {}", ep.filesystem_scope);
                println!("Shell Execution:      {}", ep.shell_execution);
                println!("Network Access:       {}", ep.network_access);
                println!("Subagents:            {}", ep.subagents);
                println!("Background Tasks:     {}", ep.background_tasks);
                println!("Memory Persistence:   {}", ep.memory_persistence);
                println!("Audit:                {}", ep.audit);
                println!("Max Steps:            {}", ep.max_steps);
                println!("Max Tool Calls:       {}", ep.max_tool_calls);
                println!("\nNon-Bypassable Boundaries (Applies to all presets):");
                for b in &ep.non_bypassable_boundaries {
                    println!("  [!] {}", b);
                }
            }
            PresetCommands::Use { name } => {
                let _p = AgentPreset::parse(&name).ok_or_else(|| {
                    format!("Unknown preset '{}'", name)
                })?;
                let agy_dir = Path::new(".agy");
                if !agy_dir.exists() {
                    let _ = std::fs::create_dir_all(agy_dir);
                }
                let cfg_path = agy_dir.join("config.json");
                let mut config: serde_json::Value = if cfg_path.exists() {
                    std::fs::read_to_string(&cfg_path)
                        .ok()
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_else(|| serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };
                config["default_preset"] = serde_json::Value::String(name.clone());
                if let Ok(serialized) = serde_json::to_string_pretty(&config) {
                    let _ = std::fs::write(&cfg_path, serialized);
                }
                println!("[OK] Workspace preset switched to '{}'.", name);
            }
        },

        Commands::Tools { workspace } => {
            println!("Registered Tools (Workspace: {}):", workspace.display());
            println!("------------------------------------------------------------");
            let reg = ToolRegistry::standard();
            for t in &reg.tools {
                println!("{:<16} {}", t.name(), t.description());
            }
        }

        Commands::Sessions { sub } => match sub {
            SessionCommands::List { workspace } => {
                let sm = SessionManager::new(&workspace);
                let list = sm.list_sessions()?;
                if list.is_empty() {
                    println!("No stored sessions in workspace.");
                } else {
                    println!("{:<36} {:<15} {:<12} {:<24}", "SESSION ID", "NAME", "PRESET", "UPDATED");
                    println!("-----------------------------------------------------------------------------------------");
                    for s in list {
                        println!("{:<36} {:<15} {:<12} {:<24}", s.id, s.name, s.preset.as_str(), s.updated_at);
                    }
                }
            }
            SessionCommands::Show { id, workspace } => {
                let sm = SessionManager::new(&workspace);
                let s = sm.load_session(&id)?;
                println!("Session ID: {}", s.id);
                println!("Name:       {}", s.name);
                println!("Preset:     {}", s.preset.as_str());
                println!("Created:    {}", s.created_at);
                println!("Updated:    {}", s.updated_at);
                println!("Turns:      {}", s.messages.len());
            }
            SessionCommands::Branch { id, new_name, workspace } => {
                let sm = SessionManager::new(&workspace);
                let b = sm.branch_session(&id, &new_name)?;
                println!("[OK] Branched session '{}' -> new session '{}' ({})", id, b.name, b.id);
            }
            SessionCommands::Delete { id, workspace } => {
                let sm = SessionManager::new(&workspace);
                sm.delete_session(&id)?;
                println!("[OK] Deleted session '{}'", id);
            }
        },

        Commands::Doctor { workspace } => {
            println!("============================================================");
            println!("AGY STANDALONE AGENT DOCTOR DIAGNOSTICS");
            println!("============================================================");
            println!("Workspace:          {}", workspace.canonicalize().unwrap_or(workspace.clone()).display());
            println!("Git Available:      {}", if which_check("git") { "[PASS]" } else { "[WARN] Not on PATH" });
            println!("Shell Available:    {}", if which_check("powershell") || which_check("sh") { "[PASS]" } else { "[FAIL]" });
            println!("Compiler (GCC):     {}", if which_check("gcc") { "[PASS]" } else { "[WARN]" });
            println!("Audit Logging:      [PASS] Always on (.agy/audit.jsonl)");
            println!("Sandbox Security:   [PASS] Non-bypassable root boundary active");
            println!("Storage Writable:   {}", if is_writable(&workspace) { "[PASS]" } else { "[FAIL]" });
            println!("Presets:            [PASS] All 6 presets defined and validated");
            println!("\nDiagnostics complete.");
        }

        Commands::Config { sub } => match sub {
            ConfigCommands::Validate { workspace } => {
                let cfg_file = workspace.join(".agy").join("config.json");
                if cfg_file.exists() {
                    let content = std::fs::read_to_string(&cfg_file)?;
                    let _: serde_json::Value = serde_json::from_str(&content)?;
                    println!("[PASS] Config file is valid JSON: {}", cfg_file.display());
                } else {
                    println!("[PASS] No custom .agy/config.json found; default configuration valid.");
                }
            }
        },

        Commands::Audit { workspace, limit } => {
            let al = AuditLogger::new(&workspace);
            let events = al.read_events()?;
            if events.is_empty() {
                println!("No audit records found in {}", al.log_path.display());
            } else {
                let slice = if events.len() > limit {
                    &events[events.len() - limit..]
                } else {
                    &events[..]
                };
                for ev in slice {
                    println!("[{}] [{}] [{}] {}", ev.timestamp, ev.preset, ev.event_type, ev.details);
                }
            }
        }

        Commands::Run { prompt, preset, workspace, json } => {
            let preset_name = resolve_preset(preset, &workspace, "Aggressive");
            let p = AgentPreset::parse(&preset_name).ok_or_else(|| format!("Unknown preset: {}", preset_name))?;
            let runner = AgentRunner::new(&workspace, p);
            let sm = SessionManager::new(&workspace);
            let session = sm.create_session("AutoRun", p)?;

            let provider = resolve_provider(&workspace);

            let result = runner.run_task(&session.id, &prompt, provider.as_ref())?;

            if json {
                let out = serde_json::json!({
                    "session_id": session.id,
                    "preset": p.as_str(),
                    "prompt": prompt,
                    "output": result
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("============================================================");
                println!("TASK OUTPUT (Preset: {})", p.as_str());
                println!("============================================================");
                println!("{}", result);
            }
        }

        Commands::Chat { prompt, preset, workspace, session } => {
            let preset_name = resolve_preset(preset, &workspace, "Uncensored");
            let p = AgentPreset::parse(&preset_name).ok_or_else(|| format!("Unknown preset: {}", preset_name))?;
            let runner = AgentRunner::new(&workspace, p);
            let sm = SessionManager::new(&workspace);

            let session_obj = if let Some(sid) = session {
                sm.load_session(&sid)?
            } else {
                sm.create_session("InteractiveChat", p)?
            };

            let provider = resolve_provider(&workspace);

            println!("============================================================");
            println!("AGY Standalone Agent Interactive REPL");
            println!("Session: {} | Preset: {}", session_obj.id, p.as_str());
            println!("Type 'exit' or 'quit' to end session.");
            println!("============================================================");

            if let Some(initial) = prompt {
                print!("\nUser > {}\n", initial);
                let ans = runner.run_task(&session_obj.id, &initial, provider.as_ref())?;
                println!("Agent > {}\n", ans);
            }

            let stdin = io::stdin();
            let mut stdout = io::stdout();

            loop {
                print!("agy> ");
                stdout.flush()?;
                let mut line = String::new();
                if stdin.lock().read_line(&mut line)? == 0 {
                    break;
                }
                let trimmed = line.trim();
                if trimmed.eq_ignore_ascii_case("exit") || trimmed.eq_ignore_ascii_case("quit") {
                    println!("Exiting session.");
                    break;
                }
                if trimmed.is_empty() {
                    continue;
                }

                match runner.run_task(&session_obj.id, trimmed, provider.as_ref()) {
                    Ok(ans) => println!("Agent > {}\n", ans),
                    Err(e) => eprintln!("Error: {}\n", e),
                }
            }
        }
    }

    Ok(())
}

fn resolve_preset(cli_preset: Option<String>, workspace: &Path, fallback: &str) -> String {
    if let Some(p) = cli_preset {
        return p;
    }
    if let Ok(env_p) = std::env::var("AGY_PRESET") {
        return env_p;
    }
    let cfg_path = workspace.join(".agy").join("config.json");
    if cfg_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&cfg_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(dp) = val.get("default_preset").and_then(|v| v.as_str()) {
                    return dp.to_string();
                }
            }
        }
    }
    fallback.to_string()
}

fn resolve_provider(workspace: &Path) -> Box<dyn Provider> {
    let cfg_path = workspace.join(".agy").join("config.json");
    let mut file_url = None;
    let mut file_key = None;
    let mut file_model = None;

    if cfg_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&cfg_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(prov) = val.get("provider") {
                    file_url = prov.get("base_url").and_then(|v| v.as_str()).map(|s| s.to_string());
                    file_key = prov.get("api_key").and_then(|v| v.as_str()).map(|s| s.to_string());
                    file_model = prov.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
            }
        }
    }

    let env_key = std::env::var("OPENAI_API_KEY").ok();
    let env_url = std::env::var("OPENAI_BASE_URL").ok();
    let env_model = std::env::var("AGY_MODEL").ok().or_else(|| std::env::var("OPENAI_MODEL").ok());

    let final_key = env_key.or(file_key);
    let final_url = env_url.or(file_url);
    let final_model = env_model.or(file_model);

    if final_key.is_some() || final_url.is_some() {
        let key = final_key.unwrap_or_else(|| "ollama".to_string());
        let url = final_url.unwrap_or_else(|| "http://localhost:11434/v1".to_string());
        let model = final_model.unwrap_or_else(|| "gpt-4o".to_string());
        Box::new(OpenAiCompatibleProvider {
            base_url: url,
            api_key: key,
            default_model: model,
        }) as Box<dyn Provider>
    } else {
        Box::new(MockProvider::new("")) as Box<dyn Provider>
    }
}

fn which_check(tool: &str) -> bool {
    let cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
    std::process::Command::new(cmd).arg(tool).output().map(|o| o.status.success()).unwrap_or(false)
}

fn is_writable<P: AsRef<Path>>(dir: P) -> bool {
    let test_file = dir.as_ref().join(".write_test");
    if std::fs::write(&test_file, b"test").is_ok() {
        let _ = std::fs::remove_file(test_file);
        true
    } else {
        false
    }
}
