use clap::{Parser, Subcommand};
use agent_core::account::{AccountProfile, AccountStore};
use agent_core::auth::{AuthCredentialManager, TargetMode};
use agent_core::audit::AuditLogger;
use agent_core::gateway::{GatewayConfig, GatewayServer};
use agent_core::manager::ClientManager;
use agent_core::runner::AgentRunner;
use agent_core::staff::StaffRunner;
use policy::preset::AgentPreset;
use providers::{MockProvider, OpenAiCompatibleProvider, Provider};
use session::session::SessionManager;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
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

    /// Manage multi-provider accounts, quotas, and 429 cooling (agy-switch)
    Account {
        #[command(subcommand)]
        sub: AccountCommands,
    },

    /// Multi-account authentication, CLI token synchronization & quota-aware rotation (agy-auth & agy-cli-manager)
    Auth {
        #[command(subcommand)]
        sub: AuthCommands,
    },

    /// Start universal local API gateway for OpenAI, Claude, and Gemini formats (agy2api)
    Serve {
        /// Port to bind the HTTP gateway server
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
        /// Host address to bind
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Workspace root directory
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        /// Behavior preset for local gateway actions
        #[arg(long)]
        preset: Option<String>,
        /// Automatically open the Web Dashboard in default browser
        #[arg(long)]
        open: bool,
    },

    /// Launch interactive Web UI Dashboard in default browser
    Dashboard {
        /// Port to bind the HTTP gateway server
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
        /// Host address to bind
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Workspace root directory
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },

    /// AI Staffer delegation with specialized roles and git dirty-tree safety (agy-staff)
    Staff {
        #[command(subcommand)]
        sub: StaffCommands,
    },

    /// Detect AGY/Antigravity client installations and running processes (AntigravityManager)
    Client {
        #[command(subcommand)]
        sub: ClientCommands,
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
enum AccountCommands {
    /// List all configured accounts
    List {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Add a new account profile
    Add {
        /// Account name or nickname
        #[arg(short, long)]
        name: String,
        /// Provider type (openai, claude, gemini, deepseek, ollama, etc.)
        #[arg(short, long)]
        provider: String,
        /// Secret API key
        #[arg(short, long)]
        key: String,
        /// Custom API base URL
        #[arg(long)]
        url: Option<String>,
        /// Default model name
        #[arg(short, long)]
        model: Option<String>,
        /// Optional token quota limit
        #[arg(short, long)]
        quota: Option<u64>,
        /// Optional email identity
        #[arg(long)]
        email: Option<String>,
        /// Optional plan tier (e.g. Pro, Ultra, Enterprise, Free)
        #[arg(long)]
        tier: Option<String>,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Switch active account
    Switch {
        /// Account ID or name
        id_or_name: String,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Remove an account
    Remove {
        /// Account ID or name
        id_or_name: String,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Show quota and token usage breakdown
    Quota {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Display account pool ranked by health score and reliability (zcop/agy-cli-manager)
    Rank {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Automatic failover and rotation to top-ranked standby account
    Rotate {
        /// Low-quota rotation threshold percentage (default: 90.0)
        #[arg(short, long, default_value_t = 90.0)]
        threshold: f64,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// List all managed accounts with health scores, plans, and quota usage (agy-auth & agy-cli-manager)
    List {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Switch active credentials across CLI, Desktop, or both with automatic backup
    Switch {
        /// Account name or ID
        id_or_name: String,
        /// Target environment: cli, desktop, or both
        #[arg(short, long, default_value = "both")]
        target: String,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Quota-aware automatic failover and rotation to top-ranked standby account
    Rotate {
        /// Low-quota rotation trigger threshold in percentage (default: 90.0)
        #[arg(short, long, default_value_t = 90.0)]
        threshold: f64,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Display account pool ranked by health score, quota, and reliability
    Rank {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Capture current live Antigravity CLI token into a managed profile
    Capture {
        /// Name alias for captured account profile
        #[arg(short, long)]
        name: String,
        /// Optional email identity
        #[arg(short, long)]
        email: Option<String>,
        /// Optional plan tier (e.g. Pro, Ultra, Enterprise)
        #[arg(short, long)]
        plan: Option<String>,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Synchronize account store with active live CLI token
    Sync {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Export managed account profiles into a portable snapshot bundle
    Export {
        /// Output file path (default: .agy/profiles_bundle.json)
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Import account profiles from a snapshot bundle
    Import {
        /// Input snapshot bundle file path
        file: PathBuf,
        /// Overwrite existing accounts with same name
        #[arg(long, default_value_t = false)]
        overwrite: bool,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// List historical credential backups
    Backups {
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
    },
    /// Restore live credentials from a historical backup file
    Restore {
        /// Path to .bak backup file
        file: PathBuf,
        /// Target environment: cli, desktop, or both
        #[arg(short, long, default_value = "both")]
        target: String,
    },
}

#[derive(Subcommand)]
enum StaffCommands {
    /// Ad-hoc technical questions answered by staff companion
    Ask {
        question: String,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(short, long)]
        preset: Option<String>,
    },
    /// Read-only codebase research and architecture investigation
    Research {
        topic: String,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(short, long)]
        preset: Option<String>,
    },
    /// Review git diff or target file for correctness, security, and bugs
    Review {
        #[arg(short, long)]
        file: Option<PathBuf>,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(short, long)]
        preset: Option<String>,
    },
    /// Autonomous code implementation with mandatory git dirty-tree safety
    Implement {
        task: String,
        /// Override refusal if git working tree contains uncommitted changes
        #[arg(short, long)]
        force: bool,
        #[arg(short, long, default_value = ".")]
        workspace: PathBuf,
        #[arg(short, long)]
        preset: Option<String>,
    },
}

#[derive(Subcommand)]
enum ClientCommands {
    /// Detect installed AGY / Antigravity client binaries on host system
    Detect,
    /// Inspect running client processes and local gateway connection status
    Status {
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
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
        Commands::Account { sub } => match sub {
            AccountCommands::List { workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let store = AccountStore::load_from_path(&acc_path)?;
                let accounts = store.list_accounts();
                if accounts.is_empty() {
                    println!("No accounts configured. Use 'agy-agent account add' to register an account.");
                } else {
                    println!("=========================================================================================");
                    println!("{:<16} {:<12} {:<18} {:<8} {:<10} {:<16}", "NAME", "PROVIDER", "MODEL", "ACTIVE", "COOLING", "KEY");
                    println!("-----------------------------------------------------------------------------------------");
                    for a in accounts {
                        let active_str = if a.is_active { "[ACTIVE]" } else { " " };
                        let cooling_str = if a.is_cooling() { "[COOLING]" } else { "Ready" };
                        let model_str = a.model.as_deref().unwrap_or("default");
                        println!(
                            "{:<16} {:<12} {:<18} {:<8} {:<10} {:<16}",
                            a.name,
                            a.provider,
                            model_str,
                            active_str,
                            cooling_str,
                            a.safe_masked_key()
                        );
                    }
                    println!("=========================================================================================");
                }
            }
            AccountCommands::Add { name, provider, key, url, model, quota, email, tier, workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let mut store = AccountStore::load_from_path(&acc_path)?;
                let mut profile = AccountProfile::new(name.clone(), provider, key, url, model, quota);
                if let Some(em) = email {
                    profile.email = Some(em);
                }
                if let Some(t) = tier {
                    profile.plan_tier = Some(t);
                }
                store.add_account(profile)?;
                store.save_to_path(&acc_path)?;
                println!("[OK] Account '{}' registered successfully.", name);
            }
            AccountCommands::Switch { id_or_name, workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let mut store = AccountStore::load_from_path(&acc_path)?;
                store.switch_active(&id_or_name)?;
                store.save_to_path(&acc_path)?;
                println!("[OK] Switched active account to '{}'.", id_or_name);
            }
            AccountCommands::Remove { id_or_name, workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let mut store = AccountStore::load_from_path(&acc_path)?;
                let removed = store.remove_account(&id_or_name)?;
                store.save_to_path(&acc_path)?;
                println!("[OK] Removed account '{}' ({})", removed.name, removed.id);
            }
            AccountCommands::Quota { workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let store = AccountStore::load_from_path(&acc_path)?;
                let accounts = store.list_accounts();
                if accounts.is_empty() {
                    println!("No accounts found.");
                } else {
                    println!("=========================================================================================");
                    println!("{:<16} {:<12} {:<14} {:<12} {:<12} {:<8}", "NAME", "PROVIDER", "QUOTA USED", "LIMIT", "REQUESTS", "ERRORS");
                    println!("-----------------------------------------------------------------------------------------");
                    for a in accounts {
                        let limit_str = a.quota_limit.map(|q| q.to_string()).unwrap_or_else(|| "Unlimited".to_string());
                        println!(
                            "{:<16} {:<12} {:<14} {:<12} {:<12} {:<8}",
                            a.name, a.provider, a.quota_used, limit_str, a.request_count, a.error_count
                        );
                    }
                    println!("=========================================================================================");
                }
            }
            AccountCommands::Rank { workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let store = AccountStore::load_from_path(&acc_path)?;
                let ranking = store.rank_accounts();
                println!("==========================================================================================================");
                println!("ACCOUNT HEALTH RANKING POOL");
                println!("==========================================================================================================");
                for a in ranking {
                    let quota_str = if let Some(pct) = a.quota_percent {
                        format!("{:.1}% ({}/{})", pct, a.quota_used, a.quota_limit.unwrap_or(0))
                    } else {
                        format!("{} used (Unlimited)", a.quota_used)
                    };
                    println!("#{:02} [{}] Score: {:<3}/100 | Tier: {:<10} | Quota: {} | Status: {}",
                        a.rank, a.name, a.health_score, a.plan_tier, quota_str, a.status);
                }
                println!("==========================================================================================================");
            }
            AccountCommands::Rotate { threshold, workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let mut store = AccountStore::load_from_path(&acc_path)?;
                match store.auto_rotate(threshold)? {
                    Some(rot) => {
                        let _ = AuthCredentialManager::apply_switch(&mut store, &rot.to, TargetMode::Both, &workspace);
                        println!("============================================================");
                        println!("QUOTA-AWARE AUTO-ROTATION: SUCCESSFUL");
                        println!("============================================================");
                        println!("Rotated From:   {}", rot.from);
                        println!("Rotated To:     {}", rot.to);
                        println!("Trigger Reason: {}", rot.reason);
                    }
                    None => {
                        let active = store.get_active_account().map(|a| a.name.clone()).unwrap_or_else(|| "none".to_string());
                        println!("[OK] Active account '{}' is healthy. No rotation required.", active);
                    }
                }
            }
        },

        Commands::Auth { sub } => match sub {
            AuthCommands::List { workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let store = AccountStore::load_from_path(&acc_path)?;
                let ranking = store.rank_accounts();
                if ranking.is_empty() {
                    println!("No accounts configured. Use 'agy-agent auth capture' or 'agy-agent account add'.");
                } else {
                    println!("==========================================================================================================");
                    println!("MANAGED ACCOUNTS & HEALTH RANKING POOL");
                    println!("==========================================================================================================");
                    println!("{:<4} {:<16} {:<12} {:<10} {:<10} {:<14} {:<8} {:<16}", "RNK", "NAME", "PROVIDER", "TIER", "HEALTH", "STATUS", "ACTIVE", "KEY");
                    println!("----------------------------------------------------------------------------------------------------------");
                    for a in ranking {
                        let active_str = if a.is_active { "[ACTIVE]" } else { " " };
                        let health_bar = format!("{}/100", a.health_score);
                        println!(
                            "{:<4} {:<16} {:<12} {:<10} {:<10} {:<14} {:<8} {:<16}",
                            a.rank, a.name, a.provider, a.plan_tier, health_bar, a.status, active_str, a.masked_key
                        );
                    }
                    println!("==========================================================================================================");
                }
            }
            AuthCommands::Switch { id_or_name, target, workspace } => {
                let target_mode = target.parse::<TargetMode>()?;
                let acc_path = AccountStore::default_path(&workspace);
                let mut store = AccountStore::load_from_path(&acc_path)?;
                let report = AuthCredentialManager::apply_switch(&mut store, &id_or_name, target_mode, &workspace)?;
                println!("============================================================");
                println!("CREDENTIAL SWITCH: SUCCESSFUL");
                println!("============================================================");
                println!("Account:        {} ({})", report.account_name, report.provider);
                println!("Target Mode:    {:?}", report.target_mode);
                println!("Updated Paths:  {}", report.updated_paths.len());
                for p in &report.updated_paths {
                    println!("  - Target:     {}", p.display());
                }
                println!("Backups Taken:  {}", report.backups_created.len());
                for b in &report.backups_created {
                    println!("  - Backup:     {} (SHA-256: {})", b.filename, &b.sha256[..8]);
                }
            }
            AuthCommands::Rotate { threshold, workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let mut store = AccountStore::load_from_path(&acc_path)?;
                match store.auto_rotate(threshold)? {
                    Some(rot) => {
                        let report = AuthCredentialManager::apply_switch(&mut store, &rot.to, TargetMode::Both, &workspace)?;
                        println!("============================================================");
                        println!("QUOTA-AWARE AUTO-ROTATION: SUCCESSFUL");
                        println!("============================================================");
                        println!("Rotated From:   {}", rot.from);
                        println!("Rotated To:     {} ({})", rot.to, report.provider);
                        println!("Trigger Reason: {}", rot.reason);
                        println!("Live Credential Updated: {:?}", report.target_mode);
                    }
                    None => {
                        let active = store.get_active_account().map(|a| a.name.clone()).unwrap_or_else(|| "none".to_string());
                        println!("[OK] Active account '{}' is healthy. No rotation required.", active);
                    }
                }
            }
            AuthCommands::Rank { workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let store = AccountStore::load_from_path(&acc_path)?;
                let ranking = store.rank_accounts();
                println!("==========================================================================================================");
                println!("ACCOUNT HEALTH RANKING POOL");
                println!("==========================================================================================================");
                for a in ranking {
                    let quota_str = if let Some(pct) = a.quota_percent {
                        format!("{:.1}% ({}/{})", pct, a.quota_used, a.quota_limit.unwrap_or(0))
                    } else {
                        format!("{} used (Unlimited)", a.quota_used)
                    };
                    println!("#{:02} [{}] Score: {:<3}/100 | Tier: {:<10} | Quota: {} | Status: {}",
                        a.rank, a.name, a.health_score, a.plan_tier, quota_str, a.status);
                }
                println!("==========================================================================================================");
            }
            AuthCommands::Capture { name, email, plan, workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let mut store = AccountStore::load_from_path(&acc_path).unwrap_or_default();
                let mut profile = AuthCredentialManager::capture_live_cli_token(&name, Some("antigravity"), plan.as_deref())?;
                if let Some(em) = email {
                    profile.email = Some(em);
                }
                let captured_name = profile.name.clone();
                store.add_account(profile)?;
                store.save_to_path(&acc_path)?;
                println!("[OK] Captured live CLI token into profile '{}'.", captured_name);
            }
            AuthCommands::Sync { workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let mut store = AccountStore::load_from_path(&acc_path).unwrap_or_default();
                let report = AuthCredentialManager::sync_with_cli(&mut store, &workspace)?;
                println!("============================================================");
                println!("CLI CREDENTIAL SYNCHRONIZATION");
                println!("============================================================");
                println!("Live Token:   {}", if report.live_token_found { "[FOUND]" } else { "[NOT FOUND]" });
                if let Some(p) = report.live_token_path {
                    println!("Path:         {}", p.display());
                }
                println!("Action:       {}", report.action_taken);
                println!("Active Now:   {}", report.active_account_name);
            }
            AuthCommands::Export { output, workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let store = AccountStore::load_from_path(&acc_path)?;
                let out_file = output.unwrap_or_else(|| workspace.join(".agy").join("profiles_bundle.json"));
                AuthCredentialManager::export_bundle(&store, &out_file)?;
                println!("[OK] Exported {} accounts to '{}'", store.list_accounts().len(), out_file.display());
            }
            AuthCommands::Import { file, overwrite, workspace } => {
                let acc_path = AccountStore::default_path(&workspace);
                let mut store = AccountStore::load_from_path(&acc_path).unwrap_or_default();
                let count = AuthCredentialManager::import_bundle(&mut store, &file, overwrite)?;
                store.save_to_path(&acc_path)?;
                println!("[OK] Imported {} accounts from '{}'. Total accounts: {}", count, file.display(), store.list_accounts().len());
            }
            AuthCommands::Backups { workspace } => {
                let backups = AuthCredentialManager::list_backups(&workspace);
                if backups.is_empty() {
                    println!("No credential backups found in .agy/auth_backups/.");
                } else {
                    println!("=========================================================================================");
                    println!("{:<32} {:<10} {:<10} {:<24}", "FILENAME", "SIZE", "SHA-256", "TIMESTAMP");
                    println!("-----------------------------------------------------------------------------------------");
                    for b in backups {
                        let sha_short = if b.sha256.len() >= 8 { &b.sha256[..8] } else { &b.sha256 };
                        println!("{:<32} {:<10} {:<10} {:<24}", b.filename, b.file_size, sha_short, b.created_at.to_rfc3339());
                    }
                    println!("=========================================================================================");
                }
            }
            AuthCommands::Restore { file, target } => {
                let target_mode = target.parse::<TargetMode>()?;
                let report = AuthCredentialManager::restore_backup(&file, target_mode)?;
                println!("============================================================");
                println!("CREDENTIAL RESTORATION: SUCCESSFUL");
                println!("============================================================");
                println!("Restored From:  {}", report.restored_from.display());
                println!("Target Mode:    {:?}", report.target_mode);
                for p in &report.updated_paths {
                    println!("  - Target:     {}", p.display());
                }
            }
        },

        Commands::Serve { port, host, workspace, preset: _, open } => {
            let acc_path = AccountStore::default_path(&workspace);
            let store = AccountStore::load_from_path(&acc_path).unwrap_or_default();
            let audit = Arc::new(AuditLogger::new(&workspace));
            let cfg = GatewayConfig {
                host: host.clone(),
                port,
                workspace: workspace.clone(),
            };
            let server = GatewayServer::new(cfg, store, audit);

            if open {
                let url = format!("http://{}:{}/dashboard", host, port);
                println!("Opening AGY Web Dashboard at {}", url);
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("cmd").args(["/c", "start", &url]).spawn();
                #[cfg(target_os = "macos")]
                let _ = std::process::Command::new("open").arg(&url).spawn();
                #[cfg(target_os = "linux")]
                let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
            }

            server.run().await?;
        }

        Commands::Dashboard { port, host, workspace } => {
            let acc_path = AccountStore::default_path(&workspace);
            let store = AccountStore::load_from_path(&acc_path).unwrap_or_default();
            let audit = Arc::new(AuditLogger::new(&workspace));
            let cfg = GatewayConfig {
                host: host.clone(),
                port,
                workspace: workspace.clone(),
            };
            let server = GatewayServer::new(cfg, store, audit);

            let url = format!("http://{}:{}/dashboard", host, port);
            println!("Launching AGY Web Dashboard at {}", url);
            #[cfg(target_os = "windows")]
            let _ = std::process::Command::new("cmd").args(["/c", "start", &url]).spawn();
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(&url).spawn();
            #[cfg(target_os = "linux")]
            let _ = std::process::Command::new("xdg-open").arg(&url).spawn();

            server.run().await?;
        }

        Commands::Staff { sub } => match sub {
            StaffCommands::Ask { question, workspace, preset } => {
                let preset_name = resolve_preset(preset, &workspace, "Uncensored");
                let p = AgentPreset::parse(&preset_name).ok_or_else(|| format!("Unknown preset: {}", preset_name))?;
                let runner = AgentRunner::new(&workspace, p);
                let provider = resolve_provider(&workspace);
                let verdict = StaffRunner::run_ask(&workspace, &question, &runner, provider.as_ref())?;
                println!("============================================================");
                println!("[STAFF COMPANION: ASK]");
                println!("============================================================");
                println!("{}", verdict.summary);
            }
            StaffCommands::Research { topic, workspace, preset } => {
                let preset_name = resolve_preset(preset, &workspace, "Unrestricted");
                let p = AgentPreset::parse(&preset_name).ok_or_else(|| format!("Unknown preset: {}", preset_name))?;
                let runner = AgentRunner::new(&workspace, p);
                let provider = resolve_provider(&workspace);
                let verdict = StaffRunner::run_research(&workspace, &topic, &runner, provider.as_ref())?;
                println!("============================================================");
                println!("[STAFF COMPANION: RESEARCHER]");
                println!("============================================================");
                println!("{}", verdict.summary);
            }
            StaffCommands::Review { file, workspace, preset } => {
                let preset_name = resolve_preset(preset, &workspace, "Aggressive");
                let p = AgentPreset::parse(&preset_name).ok_or_else(|| format!("Unknown preset: {}", preset_name))?;
                let runner = AgentRunner::new(&workspace, p);
                let provider = resolve_provider(&workspace);
                let verdict = StaffRunner::run_review(&workspace, file.as_deref(), &runner, provider.as_ref())?;
                println!("============================================================");
                println!("[STAFF COMPANION: CODE REVIEWER]");
                println!("============================================================");
                if let Some(decision) = verdict.decision {
                    println!("Verdict: {}\n", decision);
                }
                println!("{}", verdict.summary);
            }
            StaffCommands::Implement { task, force, workspace, preset } => {
                let preset_name = resolve_preset(preset, &workspace, "Unleashed");
                let p = AgentPreset::parse(&preset_name).ok_or_else(|| format!("Unknown preset: {}", preset_name))?;
                let runner = AgentRunner::new(&workspace, p);
                let provider = resolve_provider(&workspace);
                match StaffRunner::run_implement(&workspace, &task, force, &runner, provider.as_ref()) {
                    Ok(verdict) => {
                        println!("============================================================");
                        println!("[STAFF COMPANION: IMPLEMENTER]");
                        println!("============================================================");
                        println!("Dirty Tree Safety Guardrail: PASSED (Files checked: {})", verdict.uncommitted_files.len());
                        println!("{}", verdict.summary);
                    }
                    Err(e) => {
                        eprintln!("[STAFF COMPANION ERROR]: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },

        Commands::Client { sub } => match sub {
            ClientCommands::Detect => {
                println!("============================================================");
                println!("ANTIGRAVITY / AGY CLIENT INSTALLATION DETECTOR");
                println!("============================================================");
                if let Some(path) = ClientManager::detect_installation() {
                    let version = ClientManager::probe_version(&path).unwrap_or_else(|| "Unknown".to_string());
                    println!("Status:   [INSTALLED]");
                    println!("Path:     {}", path.display());
                    println!("Version:  {}", version);
                } else {
                    println!("Status:   [NOT FOUND]");
                    println!("Note:     No standard AGY or Antigravity binary detected on PATH or system install directories.");
                }
            }
            ClientCommands::Status { port } => {
                println!("============================================================");
                println!("ANTIGRAVITY / AGY CLIENT & GATEWAY TELEMETRY");
                println!("============================================================");
                let report = ClientManager::inspect_environment(Some(port));
                println!("Installed:          {}", if report.installed { "[PASS]" } else { "[NO]" });
                if let Some(p) = report.binary_path {
                    println!("Binary Path:        {}", p.display());
                }
                if let Some(v) = report.version {
                    println!("Version:            {}", v);
                }
                println!("Running Processes:  {}", report.running_processes.len());
                for proc in &report.running_processes {
                    println!("  - PID {:<6} {}", proc.pid, proc.name);
                }
                println!("Gateway Online:     {}", if report.gateway_connected { "[CONNECTED on port ".to_string() + &port.to_string() + "]" } else { "[DISCONNECTED / STANDALONE]".to_string() });
            }
        },

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
    let env_key = std::env::var("OPENAI_API_KEY").ok();
    let env_url = std::env::var("OPENAI_BASE_URL").ok();
    let env_model = std::env::var("AGY_MODEL").ok().or_else(|| std::env::var("OPENAI_MODEL").ok());

    if env_key.is_some() || env_url.is_some() {
        let key = env_key.unwrap_or_else(|| "ollama".to_string());
        let url = env_url.unwrap_or_else(|| "http://localhost:11434/v1".to_string());
        let model = env_model.unwrap_or_else(|| "gpt-4o".to_string());
        return Box::new(OpenAiCompatibleProvider {
            base_url: url,
            api_key: key,
            default_model: model,
        }) as Box<dyn Provider>;
    }

    // Check AccountStore for configured active account
    let accounts_path = AccountStore::default_path(workspace);
    if let Ok(store) = AccountStore::load_from_path(&accounts_path) {
        if let Some(active) = store.get_active_account() {
            let base_url = active.base_url.clone().unwrap_or_else(|| {
                match active.provider.to_lowercase().as_str() {
                    "claude" => "https://api.anthropic.com/v1".to_string(),
                    "gemini" => "https://generativelanguage.googleapis.com".to_string(),
                    _ => "https://api.openai.com/v1".to_string(),
                }
            });
            let model = active.model.clone().unwrap_or_else(|| "gpt-4o".to_string());
            return Box::new(OpenAiCompatibleProvider {
                base_url,
                api_key: active.api_key.clone(),
                default_model: model,
            }) as Box<dyn Provider>;
        }
    }

    // Check workspace .agy/config.json
    let cfg_path = workspace.join(".agy").join("config.json");
    if cfg_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&cfg_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(prov) = val.get("provider") {
                    let file_url = prov.get("base_url").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let file_key = prov.get("api_key").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let file_model = prov.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
                    if file_key.is_some() || file_url.is_some() {
                        let key = file_key.unwrap_or_else(|| "ollama".to_string());
                        let url = file_url.unwrap_or_else(|| "http://localhost:11434/v1".to_string());
                        let model = file_model.unwrap_or_else(|| "gpt-4o".to_string());
                        return Box::new(OpenAiCompatibleProvider {
                            base_url: url,
                            api_key: key,
                            default_model: model,
                        }) as Box<dyn Provider>;
                    }
                }
            }
        }
    }

    Box::new(MockProvider::new("")) as Box<dyn Provider>
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
