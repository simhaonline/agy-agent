use clap::{Parser, Subcommand};
use common::hash::{compute_sha256_file, compute_sha256_bytes};
use patcher_core::backup::BackupManager;
use patcher_core::download::ReleaseAcquisition;
use patcher_core::pe::PeInspection;
use patcher_core::plan::PatchPlan;
use patcher_core::profile::PatcherPreset;
use patcher_core::report::{LegacyReport, RichReport};
use patcher_core::restore::RestoreManager;
use patcher_core::rules::{get_baseline_104_rules, parse_targets_json, PatchRule};
use patcher_core::transaction::TransactionalApplier;
use patcher_core::verify::Verifier;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "agy-patcher")]
#[command(about = "AGY Binary-Patching Toolkit and Inspection Utility (Rust)", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect binary PE structure, sections, hashes, and profile compatibility
    Inspect {
        /// Target binary file
        binary: PathBuf,
        /// Output inspection report as JSON
        #[arg(long)]
        json: bool,
    },

    /// Generate a read-only patch plan with exact diffs, offsets, and warnings
    Plan {
        /// Target binary file
        binary: PathBuf,
        /// Path to custom targets.json (defaults to bundled 104 rules)
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Named patch profile preset (Heretic, Uncensored, Unrestricted, Aggressive, Abliterated, Unleashed)
        #[arg(short, long)]
        preset: Option<String>,
        /// Enable legacy Python compatibility mode
        #[arg(long)]
        compat: bool,
        /// Output plan as JSON
        #[arg(long)]
        json: bool,
    },

    /// Apply approved patch plan with transactional safety and verified backup
    Apply {
        /// Input binary file
        binary: PathBuf,
        /// Output destination binary (required unless --in-place is specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Allow in-place mutation of the input binary (creates verified .original backup)
        #[arg(long)]
        in_place: bool,
        /// Path to custom targets.json
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Named patch profile preset
        #[arg(short, long)]
        preset: Option<String>,
        /// Explicit backup path
        #[arg(long)]
        backup: Option<PathBuf>,
        /// Path to output rich JSON report
        #[arg(long)]
        report: Option<PathBuf>,
        /// Path to output legacy Python patch_report.json
        #[arg(long)]
        legacy_report: Option<PathBuf>,
        /// Enable legacy Python compatibility mode
        #[arg(long)]
        compat: bool,
    },

    /// Verify an output binary against expected hash or plan
    Verify {
        /// Binary file to verify
        binary: PathBuf,
        /// Expected SHA-256 hash
        #[arg(long)]
        expected_hash: String,
        /// Expected size in bytes
        #[arg(long)]
        expected_size: Option<u64>,
    },

    /// Restore verified baseline original backup to binary location
    Restore {
        /// Target binary file to restore
        binary: PathBuf,
        /// Explicit path to backup file (defaults to <binary>.original)
        #[arg(short, long)]
        backup: Option<PathBuf>,
    },

    /// Corrected download-and-patch workflow for official releases
    Auto {
        /// GitHub repository (default: google-antigravity/antigravity-cli)
        #[arg(long, default_value = "google-antigravity/antigravity-cli")]
        repo: String,
        /// Pinned release tag
        #[arg(long)]
        tag: Option<String>,
        /// Output patched binary path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Local mock zip archive for offline testing
        #[arg(long)]
        mock_zip: Option<PathBuf>,
        /// Named patch profile preset
        #[arg(short, long)]
        preset: Option<String>,
        /// Enable legacy compatibility mode
        #[arg(long)]
        compat: bool,
    },
}

fn load_rules(config_path: Option<&Path>) -> Result<(Vec<PatchRule>, String), Box<dyn std::error::Error>> {
    if let Some(cp) = config_path {
        let content = fs::read_to_string(cp)?;
        let hash = compute_sha256_bytes(content.as_bytes());
        let rules = parse_targets_json(&content)?;
        Ok((rules, hash))
    } else {
        let rules = get_baseline_104_rules();
        let hash = "5be6a9b28e8b44b4f5063854a49de36bb5eb5d7bf075100f0df0a982a42386f6".to_string();
        Ok((rules, hash))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Inspect { binary, json } => {
            if !binary.exists() {
                eprintln!("[!] Binary not found: {:?}", binary);
                std::process::exit(1);
            }

            let data = fs::read(&binary)?;
            let sha256 = compute_sha256_bytes(&data);
            let pe_result = PeInspection::parse(&data);

            if json {
                let out = serde_json::json!({
                    "path": binary.display().to_string(),
                    "size": data.len(),
                    "sha256": sha256,
                    "pe": pe_result.ok()
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("============================================================");
                println!("AGY BINARY INSPECTION");
                println!("============================================================");
                println!("File:      {}", binary.display());
                println!("Size:      {} bytes ({:.2} MB)", data.len(), data.len() as f64 / 1048576.0);
                println!("SHA-256:   {}", sha256);

                match pe_result {
                    Ok(pe) => {
                        println!("Format:    {}", if pe.is_64bit { "PE32+ (64-bit)" } else { "PE32 (32-bit)" });
                        println!("Machine:   0x{:04X}", pe.machine);
                        println!("Sections:  {}", pe.number_of_sections);
                        println!("Subsystem: 0x{:04X}", pe.subsystem);
                        println!("Authenticode Table: {}", if pe.has_authenticode_table { format!("Present ({} bytes)", pe.authenticode_size) } else { "None".to_string() });
                        println!("\nSection Table:");
                        for s in &pe.sections {
                            println!("  {:<8} Raw: 0x{:08X} ({} B), VMem: 0x{:08X} ({} B), Code: {}, Writable: {}",
                                s.name, s.raw_data_pointer, s.raw_data_size, s.virtual_address, s.virtual_size, s.is_code, s.is_writable);
                        }
                    }
                    Err(e) => {
                        println!("PE Inspection: Not a valid PE binary ({})", e);
                    }
                }
            }
        }

        Commands::Plan { binary, config, preset, compat, json } => {
            if !binary.exists() {
                eprintln!("[!] Binary not found: {:?}", binary);
                std::process::exit(1);
            }

            let (rules, cfg_hash) = load_rules(config.as_deref())?;
            let preset_enum = preset.as_deref().and_then(PatcherPreset::parse);
            let data = fs::read(&binary)?;

            let plan = PatchPlan::build(&data, &rules, preset_enum, &cfg_hash, compat, true);

            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                println!("============================================================");
                println!("AGY PATCH PLAN");
                println!("============================================================");
                println!("{}", plan.to_summary());
                if !plan.warnings.is_empty() {
                    println!("\nWarnings:");
                    for w in &plan.warnings {
                        println!("  [!] {}", w);
                    }
                }
            }
        }

        Commands::Apply { binary, output, in_place, config, preset, backup, report, legacy_report, compat } => {
            if !binary.exists() {
                eprintln!("[!] Input binary not found: {:?}", binary);
                std::process::exit(1);
            }

            if !in_place && output.is_none() {
                eprintln!("[!] Specify either --output <path> or --in-place");
                std::process::exit(1);
            }

            let target_out = if in_place {
                binary.clone()
            } else {
                output.unwrap()
            };

            let (rules, cfg_hash) = load_rules(config.as_deref())?;
            let preset_enum = preset.as_deref().and_then(PatcherPreset::parse);

            // 1. Create verified backup if modifying in-place
            if in_place {
                println!("[1] BACKUP");
                let b_rec = BackupManager::create_verified_backup(&binary, backup.as_ref())?;
                println!("  [+] Verified backup at: {}", b_rec.backup_path.display());
            }

            // 2. Read baseline bytes
            let in_data = fs::read(&binary)?;
            let orig_hash = compute_sha256_bytes(&in_data);

            // 3. Generate plan
            println!("\n[2] PLANNING & SIMULATION");
            let plan = PatchPlan::build(&in_data, &rules, preset_enum, &cfg_hash, compat, !compat);
            println!("  [+] Total proposed events: {}", plan.events.len());
            println!("  [+] Changed events:        {}", plan.metrics.changed_events);
            println!("  [+] No-op events:         {}", plan.metrics.noop_events);
            println!("  [+] Expected output SHA:   {}", plan.expected_output_hash);

            // 4. Execute sequential engine to generate output bytes
            let active_rules = if let Some(p) = preset_enum {
                p.filter_rules(&rules)
            } else {
                rules
            };
            let (out_bytes, events, metrics) = patcher_core::engine::PatchEngine::simulate(&in_data, &active_rules);

            // 5. Transactional write to target
            println!("\n[3] COMMITTING PATCH");
            let committed_path = TransactionalApplier::apply(&target_out, &out_bytes, &plan.expected_output_hash)?;
            println!("  [+] Written successfully: {}", committed_path.display());

            // 6. Reports
            let new_hash = compute_sha256_bytes(&out_bytes);
            if let Some(ref lr_path) = legacy_report {
                let l_rep = LegacyReport::from_events(&events, &orig_hash, &new_hash);
                l_rep.write_to_file(lr_path)?;
                println!("  [+] Legacy report written: {}", lr_path.display());
            } else if compat {
                let default_legacy = Path::new("patch_report.json");
                let l_rep = LegacyReport::from_events(&events, &orig_hash, &new_hash);
                l_rep.write_to_file(default_legacy)?;
                println!("  [+] Legacy report written: {}", default_legacy.display());
            }

            if let Some(ref r_path) = report {
                let rich = RichReport {
                    tool_version: "0.1.0".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    input_path: binary.display().to_string(),
                    output_path: target_out.display().to_string(),
                    orig_hash,
                    new_hash,
                    preset,
                    metrics,
                    events,
                    warnings: plan.warnings,
                };
                rich.write_to_file(r_path)?;
                println!("  [+] Rich report written:   {}", r_path.display());
            }

            println!("\nPatch apply complete.");
        }

        Commands::Verify { binary, expected_hash, expected_size } => {
            let res = Verifier::verify_file(&binary, &expected_hash, expected_size)?;
            if res.is_valid {
                println!("[OK] {}", res.message);
            } else {
                eprintln!("[FAIL] {}", res.message);
                std::process::exit(1);
            }
        }

        Commands::Restore { binary, backup } => {
            println!("[*] Restoring baseline backup...");
            let restored = RestoreManager::restore(&binary, backup.as_ref())?;
            println!("[OK] Successfully restored: {}", restored.display());
        }

        Commands::Auto { repo, tag, output, mock_zip, preset, compat } => {
            println!("============================================================");
            println!("AGY AUTOMATIC RELEASE ACQUISITION & PATCH PIPELINE");
            println!("============================================================");

            let temp_dir = tempfile::tempdir()?;
            let zip_bytes = if let Some(ref mock) = mock_zip {
                println!("[*] Using local mock archive: {}", mock.display());
                fs::read(mock)?
            } else {
                let tag_str = tag.unwrap_or_else(|| "latest".to_string());
                let download_url = format!(
                    "https://github.com/{}/releases/download/{}/agy_cli_windows_x64.zip",
                    repo, tag_str
                );
                println!("[*] Fetching release from: {}", download_url);
                ReleaseAcquisition::fetch_asset_bytes(&download_url).await?
            };

            println!("[*] Extracting antigravity.exe securely...");
            let extracted = ReleaseAcquisition::extract_member_safe(
                &zip_bytes,
                "antigravity.exe",
                temp_dir.path(),
                500 * 1024 * 1024,
            )?;
            let orig_hash = compute_sha256_file(&extracted)?;
            println!("  [+] Extracted executable SHA-256: {}", orig_hash);

            let out_target = output.unwrap_or_else(|| PathBuf::from("antigravity-patched.exe"));

            let (rules, cfg_hash) = load_rules(None)?;
            let preset_enum = preset.as_deref().and_then(PatcherPreset::parse);
            let in_data = fs::read(&extracted)?;

            let plan = PatchPlan::build(&in_data, &rules, preset_enum, &cfg_hash, compat, !compat);
            println!("  [+] Generated plan: {} events proposed", plan.events.len());

            let active_rules = if let Some(p) = preset_enum {
                p.filter_rules(&rules)
            } else {
                rules
            };
            let (out_bytes, events, _metrics) = patcher_core::engine::PatchEngine::simulate(&in_data, &active_rules);

            TransactionalApplier::apply(&out_target, &out_bytes, &plan.expected_output_hash)?;
            println!("[OK] Successfully applied patch to: {}", out_target.display());

            if compat {
                let l_rep = LegacyReport::from_events(&events, &orig_hash, &plan.expected_output_hash);
                l_rep.write_to_file("patch_report.json")?;
                println!("  [+] Legacy report written to patch_report.json");
            }
        }
    }

    Ok(())
}
