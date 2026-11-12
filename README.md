# AGY: Standalone Rust Agent Runtime & Binary-Patching Toolkit

[![Rust Version](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform: Windows x64](https://img.shields.io/badge/platform-Windows%20x64-lightgrey.svg)](https://microsoft.com/windows)
[![Platform: Linux x64/x86 MUSL](https://img.shields.io/badge/platform-Linux%20MUSL-lightgrey.svg)](https://alpinelinux.org/)
[![Tests](https://img.shields.io/badge/tests-48%20passed%2C%200%20failed-brightgreen.svg)]()

A high-performance, memory-safe Rust Cargo workspace delivering two strictly separated, independently usable software products:

1. **`agy-patcher`**: A verified binary-patching and PE inspection toolkit featuring transactional staging, verified backups, crash-safe atomic rollbacks, bounds-checked PE32/PE32+ parsing, and exact 104-rule compatibility emulation.
2. **`agy-agent`**: A standalone autonomous agent runtime with model-provider abstraction, persistent session and memory management, sandboxed tool execution, automated credential redaction, tamper-evident audit logging, subagent delegation, and six named behavior/permission presets.

---

## ⚡ Quick Install (Single-Line Command)

Install `agy-agent` and `agy-patcher` instantly across your systems:

### Linux (Ubuntu, Debian, Fedora, Arch, Alpine - x86_64 & i686)
```bash
curl -fsSL https://raw.githubusercontent.com/simhaonline/agy-agent/main/install.sh | bash
```

### Windows 10 / 11 / Server (PowerShell)
```powershell
irm https://raw.githubusercontent.com/simhaonline/agy-agent/main/install.ps1 | iex
```

---

## Quick Navigation & Enterprise Documentation

- 📘 **[Admin & User Guide](docs/ADMIN_USER_GUIDE.md)**: Comprehensive guide covering deployment, interactive chat, autonomous goal running, central `.agy/config.json` schema, policy enforcement, SIEM integration, and troubleshooting.
- 📦 **Pre-Compiled Production Binaries**: Ready to use standalone executables located in the [`dist/`](dist/) folder:
  - **Windows (x64)**: [`dist/windows-x64/agy-patcher.exe`](dist/windows-x64/agy-patcher.exe) & [`dist/windows-x64/agy-agent.exe`](dist/windows-x64/agy-agent.exe)
  - **Linux (x64 MUSL - Static)**: [`dist/linux-x64/agy-patcher`](dist/linux-x64/agy-patcher) & [`dist/linux-x64/agy-agent`](dist/linux-x64/agy-agent)
  - **Linux (x86 MUSL - Static)**: [`dist/linux-x86/agy-patcher`](dist/linux-x86/agy-patcher) & [`dist/linux-x86/agy-agent`](dist/linux-x86/agy-agent)

---

## Table of Contents

- [1. Architectural Separation](#1-architectural-separation)
- [2. Workspace Organization](#2-workspace-organization)
- [3. AGY Patching Toolkit (`agy-patcher`)](#3-agy-patching-toolkit-agy-patcher)
  - [Key Capabilities](#key-capabilities)
  - [Commands & How to Use](#commands--how-to-use)
  - [Safety & Transaction Lifecycle](#safety--transaction-lifecycle)
- [4. Standalone Agent Runtime (`agy-agent`)](#4-standalone-agent-runtime-agy-agent)
  - [Key Capabilities](#key-capabilities-1)
  - [Commands & How to Use](#commands--how-to-use-1)
  - [Built-In Tool Registry](#built-in-tool-registry)
  - [Subagent Orchestration](#subagent-orchestration)
- [5. Behavior & Permission Presets Deep Dive](#5-behavior--permission-presets-deep-dive)
  - [The Six Named Presets](#the-six-named-presets)
  - [16-Dimension Effective Policy Matrix](#16-dimension-effective-policy-matrix)
  - [Non-Bypassable Security Boundaries](#non-bypassable-security-boundaries)
- [6. AI Model Provider Integration](#6-ai-model-provider-integration)
  - [Universal OpenAI-Compatible Architecture](#universal-openai-compatible-architecture)
  - [Local & Private AI (Ollama, vLLM, LM Studio)](#local--private-ai-ollama-vllm-lm-studio)
  - [Commercial Cloud AI (OpenAI, OpenRouter, DeepSeek, Groq, Mistral)](#commercial-cloud-ai-openai-openrouter-deepseek-groq-mistral)
  - [Enterprise Gateways (LiteLLM Proxy)](#enterprise-gateways-litellm-proxy)
  - [Configuration File (`.agy/config.json`)](#configuration-file-agyconfigjson)
  - [Environment Variables & CLI Overrides](#environment-variables--cli-overrides)
  - [Preset & Model Alignment Guide](#preset--model-alignment-guide)
  - [Persistent Memory (`.agy/memory.json`)](#persistent-memory-agymemoryjson)
  - [Tamper-Evident Audit Log (`.agy/audit.jsonl`)](#tamper-evident-audit-log-agyauditjsonl)
- [7. 104-Rule Engine & Synthetic Oracle Parity](#7-104-rule-engine--synthetic-oracle-parity)
  - [The 104-Rule Inventory](#the-104-rule-inventory)
  - [Telemetry No-Op Rules](#telemetry-no-op-rules)
  - [Static Shadow Conflicts](#static-shadow-conflicts)
  - [Appendix D Oracle Parity Verification](#appendix-d-oracle-parity-verification)
- [8. Building and Testing](#8-building-and-testing)
  - [Prerequisites](#prerequisites)
  - [Running Workspace Tests](#running-workspace-tests)
  - [Compiling Production Binaries](#compiling-production-binaries)
- [9. Troubleshooting & FAQ](#9-troubleshooting--faq)
- [10. Compliance, Legal & Provenance Notice](#10-compliance-legal--provenance-notice)

---

## 1. Architectural Separation

As mandated by design specifications, **the patcher and agent are strictly isolated from each other**:

- **The Standalone Agent never patches binaries**: It does not inspect PE headers, does not rewrite byte arrays, and cannot modify executable files.
- **The Patcher never launches or executes the Agent**: It has no knowledge of LLM prompts, agent task loops, sessions, or subagents.
- **Presets are Behavior & Permission Configurations**: The names *Heretic*, *Uncensored*, *Unrestricted*, *Aggressive*, *Abliterated*, and *Unleashed* configure local autonomy, sandboxing, and approval thresholds. They **do not modify model weights** and **do not bypass remote provider policies**.

```
+-------------------------------------------------------------------------------+
|                             CARGO WORKSPACE ROOT                              |
+---------------------------------------+---------------------------------------+
                                        |
                 +----------------------+----------------------+
                 |                                             |
+--------------------------------+             +--------------------------------+
|          agy-patcher           |             |           agy-agent            |
|   (Binary-Patching Toolkit)    |             |   (Autonomous Agent Runtime)   |
+--------------------------------+             +--------------------------------+
| - PE32/PE32+ Header Parser     |             | - Model Provider Abstraction   |
| - 104-Rule Sequential Scanner  |             | - Sandboxed Tool Execution     |
| - Transactional File Applier   |             | - 6 Behavior/Permission Presets|
| - Verified Backup & Restore    |             | - Session & Memory Manager     |
| - ZipSlip Safe Release Auto    |             | - Tamper-Evident Audit Logger  |
+--------------------------------+             +--------------------------------+
                 |                                             |
                 +----------------------+----------------------+
                                        |
+-------------------------------------------------------------------------------+
|                       crates/common (Shared Primitives)                       |
|   - Latin-1 Byte Encoder/Decoder   - SHA-256 Hasher   - Path Confinement      |
+-------------------------------------------------------------------------------+
```

---

## 2. Workspace Organization

The workspace is organized into nine specialized member crates under `crates/`:

| Crate | Purpose | Key Artifacts |
|---|---|---|
| [`common`](crates/common) | Shared zero-dependency utilities: Latin-1 encoding/decoding, SHA-256 calculation, and workspace path containment. | `encode_latin1`, `compute_sha256_file`, `ensure_within_root` |
| [`patcher-core`](crates/patcher-core) | Core patching library: 104-rule engine, pure-Rust PE parser, transactional file applier, verified backup/restore, download acquisition. | `PatchEngine`, `PeInspection`, `TransactionalApplier`, `BackupManager` |
| [`patcher-cli`](crates/patcher-cli) | CLI frontend binary for binary patching operations. | Binary: `agy-patcher.exe` |
| [`policy`](crates/policy) | Behavior and permission preset schemas, 16-dimension policy engine, and non-bypassable security invariants. | `AgentPreset`, `PolicyEngine`, `EffectivePolicy` |
| [`tools`](crates/tools) | Tool trait, sandboxed workspace filesystem tools, shell execution, git tools, and automated credential redaction. | `Sandbox`, `ToolRegistry`, `FileReadTool`, `ShellExecTool` |
| [`providers`](crates/providers) | LLM provider abstraction supporting OpenAI-compatible APIs, local mock provider, and offline fallback. | `Provider`, `OpenAiCompatibleProvider`, `MockProvider` |
| [`session`](crates/session) | Session persistence, persistent memory key-value store, and sliding-window context compaction. | `SessionManager`, `MemoryManager`, `compact_messages` |
| [`agent-core`](crates/agent-core) | Autonomous task runner loop, tamper-evident JSONL audit logging, subagents, and background task manager. | `AgentRunner`, `AuditLogger`, `SubagentOrchestrator` |
| [`agent-cli`](crates/agent-cli) | CLI frontend binary for interactive chat, autonomous task runs, diagnostics, and audit log inspection. | Binary: `agy-agent.exe` |

---

## 3. AGY Patching Toolkit (`agy-patcher`)

`agy-patcher` provides a memory-safe, bounds-checked workflow for inspecting, dry-run planning, transactionally applying, verifying, and atomically restoring approved PE binaries.

### Key Capabilities

- **Exact Byte Preservation**: Replacements maintain the exact byte length of the search target via space padding (`0x20`) or truncation, guaranteeing zero section drift.
- **Pure-Rust PE Inspection**: DOS MZ validation, PE32/PE32+ optional headers, section boundaries (`.text`, `.rdata`), and Authenticode certificate table detection without C dependencies.
- **Transactional Staging**: Patched files are written to a temporary staging file (`.agy_stage_<uuid>`) on the target volume, flushed and synced (`fsync`), verified against in-memory SHA-256, and atomically moved into place.
- **Verified Backups**: Always verifies source bytes before creating `<binary>.original`. Detects and rejects stale backups from different binaries.
- **Shadow Conflict Detection**: Identifies rules where earlier replacements mask or alter subsequent search targets (e.g., `allow_agent_access_gitignore_files` within `_allow_agent_access_gitignore_files`).
- **Telemetry Accounting**: Explicitly accounts for 8 telemetry rules where replacement matches search, reporting them cleanly as no-ops rather than failures.
- **Safe Release Acquisition**: Downloads official pinned releases, checks archive integrity, validates against ZipSlip path traversal, extracts only designated executable members, and validates SHA-256.

### Commands & How to Use

#### 1. Inspect a Binary (`inspect`)
Analyze the PE structure, architecture, sections, code boundaries, Authenticode signature directory, and SHA-256 hash:
```bash
# Human-readable inspection output
agy-patcher inspect path/to/application.exe

# Machine-readable JSON output
agy-patcher inspect path/to/application.exe --json
```

#### 2. Dry-Run Patch Planning (`plan`)
Simulate patching without writing any bytes to disk. Emits proposed diffs, affected byte offsets, rule hit counts, and shadow warnings:
```bash
# Default plan using the 104 bundled rules
agy-patcher plan path/to/application.exe

# Plan using a specific preset profile (e.g. Uncensored or Heretic)
agy-patcher plan path/to/application.exe --preset Uncensored

# Plan using custom targets.json file
agy-patcher plan path/to/application.exe --config my_targets.json

# Export plan as structured JSON
agy-patcher plan path/to/application.exe --json > plan.json
```

#### 3. Apply Patches Transactionally (`apply`)
Apply approved patches safely. Requires either `--in-place` (creates a verified `.original` backup) or `--output <NEW_FILE>`:
```bash
# Safe out-of-place patching (leaves original untouched)
agy-patcher apply path/to/application.exe --output path/to/application_patched.exe

# In-place patching (creates path/to/application.exe.original backup)
agy-patcher apply path/to/application.exe --in-place

# Generate Python-compatible legacy report (patch_report.json)
agy-patcher apply path/to/application.exe --in-place --legacy-report patch_report.json

# Generate rich structured JSON report with metrics and PE section events
agy-patcher apply path/to/application.exe --in-place --report rich_report.json
```

#### 4. Verify a Binary (`verify`)
Verify an output binary against its expected SHA-256 cryptographic hash and expected file size:
```bash
agy-patcher verify path/to/application_patched.exe \
  --expected-hash a0b580744b68775e529093bd99444b6cd1af0a0aa4445683384e210275c99980

# Verify both hash and exact byte size
agy-patcher verify path/to/application_patched.exe \
  --expected-hash a0b580744b68775e529093bd99444b6cd1af0a0aa4445683384e210275c99980 \
  --expected-size 153207960
```

#### 5. Restore From Backup (`restore`)
Atomically restore a binary from its verified `.original` baseline backup:
```bash
# Restore default <binary>.original backup
agy-patcher restore path/to/application.exe

# Restore from explicit custom backup path
agy-patcher restore path/to/application.exe --backup custom_backup.bin
```

#### 6. Safe Automated Download and Patch (`auto`)
Acquire an official release archive, extract with ZipSlip traversal protection, plan, patch, and verify:
```bash
# Full automated pipeline
agy-patcher auto --tag v1.0.9 --output ./bin/application_patched.exe

# Offline testing using a local mock archive
agy-patcher auto --mock-zip test_fixtures/release.zip --output ./bin/application_patched.exe
```

### Safety & Transaction Lifecycle

```
[Target File]
      |
      v
1. BACKUP CHECK  --> Creates <target>.original if absent; verifies SHA-256 matches.
      |
      v
2. SIMULATION    --> Evaluates all 104 rules in memory; generates expected SHA-256.
      |
      v
3. ATOMIC STAGE  --> Writes patched buffer to `.agy_stage_<uuid>` on the same volume.
      |              Flushes buffer & calls fsync() to guarantee disk persistence.
      v
4. VERIFICATION  --> Re-computes SHA-256 of staged file; compares against simulation.
      |              [If mismatch: deletes staged file and aborts with zero changes]
      v
5. ATOMIC MOVE   --> Renames staged file over target executable atomically.
```

---

## 4. Standalone Agent Runtime (`agy-agent`)

`agy-agent` is an autonomous coding and task agent runtime equipped with provider abstraction, session persistence, sandboxed tools, and behavior/permission presets.

### Key Capabilities

- **Strict Path Containment**: Every file tool enforces strict sandbox boundary validation. Symbolic links and relative traversals (`../../`) fail closed.
- **Automated Secret Redaction**: Intercepts tool inputs, command outputs, and model prompts to redact API keys (`sk-*`), GitHub tokens (`ghp_*`), bearer tokens, and RSA/EC private keys.
- **Persistent State**: Sessions saved in `.agy/sessions/`, long-term memory facts stored in `.agy/memory.json`, and automatic context window compaction.
- **Tamper-Evident Audit Logging**: Every command execution, tool use, approval check, and model message is logged in append-only `.agy/audit.jsonl`.
- **Subagent Delegation**: Isolated worker agents (Planner, Coder, Reviewer, Researcher, BrowserOperator) execute with strictly bounded task contracts.

### Commands & How to Use

#### 1. Interactive Chat (`chat`)
Start an interactive conversational session or ask a single question:
```bash
# Start interactive conversational REPL (type 'exit' or 'quit' to end)
agy-agent chat

# Start chat with a specific behavior preset
agy-agent chat --preset Heretic

# Pass an initial prompt directly
agy-agent chat "Explain the architecture of this Rust workspace"

# Resume an existing session
agy-agent chat --session 2dbf736e-aa73-40e0-8b6b-797644adcb50
```

#### 2. Autonomous Task Execution (`run`)
Execute an autonomous multi-step objective with tool planning, self-correction, and step budgeting:
```bash
# Run task with default Aggressive preset (up to 100 autonomous steps)
agy-agent run "Audit the codebase for unused dependencies and run tests"

# Run task under Unleashed preset (up to 200 autonomous steps)
agy-agent run "Implement integration test for backup recovery" --preset Unleashed

# Output structured JSON result for CI/CD pipelines
agy-agent run "Verify workspace compilation" --json
```

#### 3. Preset Management (`preset`)
Inspect and select behavior and permission presets:
```bash
# List all 6 available presets
agy-agent preset list

# Display full 16-dimension effective policy for a preset
agy-agent preset show Heretic
agy-agent preset show Unleashed

# Set the default workspace preset
agy-agent preset use Aggressive
```

#### 4. List Registered Tools (`tools`)
Inspect available sandboxed tools and active permission rules:
```bash
agy-agent tools
agy-agent tools --workspace /custom/path
```

#### 5. Session Management (`sessions`)
List, inspect, branch, or delete persistent conversation sessions:
```bash
# List all stored sessions
agy-agent sessions list

# Show full conversation history for a session
agy-agent sessions show <SESSION_ID>

# Branch a session to explore an alternate solution path
agy-agent sessions branch <SESSION_ID> "Experimental Branch"

# Delete a session
agy-agent sessions delete <SESSION_ID>
```

#### 6. System Diagnostics (`doctor`)
Perform environment, compiler, git, security sandbox, and storage health checks:
```bash
agy-agent doctor
```

#### 7. Configuration Validation (`config`)
Validate syntax and keys in `.agy/config.json`:
```bash
agy-agent config validate
```

#### 8. Audit Log Inspection (`audit`)
Inspect recent tamper-evident audit records from `.agy/audit.jsonl`:
```bash
# Show last 20 audit events
agy-agent audit

# Show last 50 audit events
agy-agent audit --limit 50
```

#### 9. Multi-Account Management, Quota Engine & Auth Synchronizer (`account` & `auth`)
Manage multiple API accounts, live Antigravity CLI credentials, automatic pre-switch backups, intelligent health-ranked pool rotation, and bidirectional CLI/Desktop synchronization (inspired by `agy-switch`, `fikrilal/agy-auth`, and `zcop/agy-cli-manager`, built from scratch in pure Rust):
```bash
# 1. List accounts with dynamic Health Scores, Plan Tiers (Free, Pro, Ultra, Enterprise), and Quota usage
agy-agent auth list
agy-agent account list

# 2. Seamlessly switch active credentials across CLI, Desktop, or both with automated pre-switch backup
agy-agent auth switch "Work-Claude" --target both
agy-agent account switch "OpenAI-Main"

# 3. Quota-aware automatic pool rotation (rotates when quota exceeds threshold or on HTTP 429)
agy-agent auth rotate --threshold 90.0

# 4. View account pool ranked by health score, quota headroom, and reliability
agy-agent auth rank

# 5. Capture active Google Antigravity CLI OAuth token into a named profile
agy-agent auth capture --name "Google-Personal" --email "dev@gmail.com" --plan "Pro"

# 6. Synchronize account store with active live CLI credentials
agy-agent auth sync

# 7. Export / Import portable snapshot bundles with SHA-256 integrity signatures
agy-agent auth export --output .agy/profiles_bundle.json
agy-agent auth import .agy/profiles_bundle.json --overwrite

# 8. List and restore historical credential backups
agy-agent auth backups
agy-agent auth restore .agy/auth_backups/cli_Work-Claude_20261112_143000_a1b2c3d4.bak --target both

# 9. Register standard OpenAI/Claude/Gemini/DeepSeek/Ollama API accounts
agy-agent account add --name "Claude-Work" --provider "claude" --key "sk-ant-api03-..." --model "claude-3-5-sonnet" --quota 250000 --tier "Enterprise"
agy-agent account add --name "OpenAI-Backup" --provider "openai" --key "sk-proj-..." --model "gpt-4o" --tier "Pro"

# 10. View quota usage, remaining limits, request counts, and error telemetry
agy-agent account quota
```

> [!NOTE]
> **Credential Safety & Pre-Switch Backups:** Before modifying live credentials in `~/.gemini/antigravity-cli/antigravity-oauth-token` or Desktop profiles, `agy-agent` automatically writes a timestamped, SHA-256-verified backup to `.agy/auth_backups/`. All live credential updates use atomic staged writes (`.agy_token_stage_<uuid>` + `fsync` + atomic rename) to guarantee zero file corruption.

#### 10. Universal Local API Gateway (`serve`)
Start a pure Rust async HTTP API gateway that translates incoming requests from any AI tool or IDE extension (Cursor, Cline, Continue, Aider, Open-WebUI, or curl) into AGY agent executions or upstream providers (inspired by `agy2api`):
```bash
# Start local API gateway on default port 8080 (http://127.0.0.1:8080)
agy-agent serve

# Bind to custom port and host interface
agy-agent serve --host 127.0.0.1 --port 9000 --preset Aggressive
```

**Supported Protocol Endpoints:**
- **OpenAI Compatible**: `POST /v1/chat/completions` & `GET /v1/models`
- **Anthropic Claude Compatible**: `POST /v1/messages`
- **Google Gemini Compatible**: `POST /v1beta/models/{model}:generateContent`
- **Gateway Telemetry & Health**: `GET /health` & `GET /status`
- **CORS Preflight**: Fully enabled (`OPTIONS *`) for web and browser IDE integrations.
- **Automatic 429 Cooling & Load Balancing**: When any upstream account encounters HTTP 429 or rate limits, the gateway automatically marks that account as cooling for 60s, logs an audit record, and immediately fails over to the next healthy account in the pool without dropping your request!

#### 11. AI Staffer Companion & Git Safety Guardrail (`staff`)
Delegate tasks to specialized developer staff roles with built-in protection against clobbering uncommitted work (inspired by `agy-staff`):
```bash
# 1. Ask Role: Rapid technical Q&A grounded in repository context
agy-agent staff ask "Explain the data flow of the transaction lifecycle"

# 2. Researcher Role: Read-only architecture investigation (strictly avoids file modifications)
agy-agent staff research "Inspect crypto modules and check if SHA-256 uses constant-time comparison"

# 3. Reviewer Role: Structured code review of git diffs or specific files
agy-agent staff review
agy-agent staff review --file crates/agent-core/src/account.rs

# 4. Implementer Role: Code implementation with mandatory Git Dirty-Tree Safety Guardrail
agy-agent staff implement "Add exponential backoff retry to gateway connector"

# Override refusal if working tree contains uncommitted changes
agy-agent staff implement "Refactor session store" --force
```

> [!IMPORTANT]
> **Git Dirty-Tree Safety Guardrail:** The `implement` staffer checks `git status --porcelain` before executing. If you have uncommitted changes in your repository, the staffer refuses to run unless `--force` is explicitly provided. This prevents autonomous agents from accidentally overwriting your in-progress work!

#### 12. Host Client & Process Inspector (`client`)
Detect host Antigravity/AGY client installations, inspect binary metadata, monitor running client processes, and verify gateway connectivity (inspired by `AntigravityManager`):
```bash
# Detect client installation paths (AppData, /usr/local/bin, PATH) and version
agy-agent client detect

# Inspect running AGY client processes (PID, status) and gateway connection
agy-agent client status
agy-agent client status --port 8080
```

#### 13. Interactive Web UI/UX Dashboard (`dashboard` & `serve --open`)
Access the modern, dark glassmorphic web dashboard directly in your browser. Seamlessly integrates Multi-Account switching, Gateway status, Host Client process telemetry, AI Staff companion playground, and live tamper-evident audit streaming:
```bash
# Launch dashboard server and automatically open your default browser
agy-agent dashboard

# Bind dashboard to custom port
agy-agent dashboard --port 8080 --host 127.0.0.1

# Alternatively start the gateway server with automatic browser launch
agy-agent serve --open
```

**Dashboard Features & User Experience:**
- 🎛️ **Live System Overview:** Instant status cards for active provider/model, quota usage %, active cooling status, and client detection.
- 👥 **Multi-Account Manager:** Visual cards for every registered provider (OpenAI, Claude, Gemini, DeepSeek, Ollama). Switch active account with 1 click, track token usage bars, or add new keys via modal dialog.
- 🛡️ **Host Client & Process Inspector:** Detects local AGY client installation, parses version info, displays active process PIDs, and live gateway port connectivity.
- 🤖 **Interactive AI Staff Companion:** Direct in-browser chat playground with instant role selection (`Ask`, `Researcher`, `Reviewer`, `Implementer`) and behavior presets (`Aggressive`, `Heretic`, `Uncensored`, `Unrestricted`, `Abliterated`, `Unleashed`).
- 📜 **Tamper-Evident Audit Feed:** Real-time stream of `.agy/audit.jsonl` events showing exact timestamps, tools called, redacted secrets, and execution results.
- 🔌 **Instant IDE Connect Snippets:** Ready-to-copy configuration snippets for Cursor (`~/.cursor/settings.json`), Cline, Continue, Aider, Python SDK, and curl.
- ⚡ **Zero External Dependencies:** Built with pure Rust single-binary embedding (`include_str!`). No NodeJS, npm, webpack, or static asset directories required.

---

### Built-In Tool Registry

All tools execute within the safe sandbox boundary and redact secrets automatically:

| Tool Name | Parameters | Description |
|---|---|---|
| `file_read` | `{"path": "string"}` | Reads text content from a file inside the workspace root. |
| `file_write` | `{"path": "string", "content": "string"}` | Writes text content to a file inside the workspace, creating parent directories. |
| `file_edit` | `{"path": "string", "target": "string", "replacement": "string"}` | Atomically replaces exact matching target text with replacement text in a file. |
| `dir_list` | `{"path": "string"}` *(optional)* | Lists directory entries (files and subdirectories) within the workspace. |
| `shell_exec` | `{"command": "string"}` | Executes a shell command inside the workspace directory subject to policy rules. |
| `git_status` | `{}` | Returns `git status --short` for the workspace repository. |
| `git_diff` | `{}` | Returns `git diff` working tree modifications. |

---

### Subagent Orchestration

`agy-agent` orchestrates specialized subagents to divide complex tasks:

- **Planner**: Decomposes high-level prompts into MECE milestones and step budgets.
- **Coder**: Implements code changes, refactorings, and file edits.
- **Reviewer**: Evaluates code quality, lint status, and verifies that no regressions were introduced.
- **Researcher**: Searches documentation, codebase symbols, and files.
- **BrowserOperator**: Coordinates external browser or web actions (if authorized).

Each subagent runs under an isolated **`SubagentTaskContract`** defining its role, step budget, allowed tools, and distinct context window to prevent context rot.

---

## 5. Behavior & Permission Presets Deep Dive

### The Six Named Presets

1. **`Heretic`**: Focuses on direct, technical responses with minimal stylistic friction, high density of code, and no conversational filler.
2. **`Uncensored`**: Eliminates all local, client-side keyword refusal heuristics. Requests are evaluated strictly against provider capabilities, non-bypassable safety boundaries, and the law.
3. **`Unrestricted`**: Broadens tool access, grant permissions for filesystem and shell operations within the workspace root, and increases step limits.
4. **`Aggressive`**: Proactive, autonomous execution with high step budgets (100 steps), automated error retries, and subagent delegation.
5. **`Abliterated`**: Maximally permissive local policy configuration. Bypasses confirmation prompts for workspace-internal actions while strictly upholding all non-bypassable boundaries. *(Note: This is a runtime permission profile, not a model weight modification).*
6. **`Unleashed`**: Maximum autonomy profile. Allocates up to 200 autonomous steps, full tool availability, subagent chaining, and persistent session memory.

---

### 16-Dimension Effective Policy Matrix

| Policy Dimension | Heretic | Uncensored | Unrestricted | Aggressive | Abliterated | Unleashed |
|---|---|---|---|---|---|---|
| **1. Persona / Style** | Direct, technical | Objective, factual | Full technical | Proactive, fast | Unfiltered tone | Maximum output |
| **2. Client Friction** | Low | None | Low | Low | None | None |
| **3. Planning Autonomy** | Medium | Medium | High | High | High | Maximum |
| **4. Tool Permissions** | Medium | Medium | Full | High | Full | Full |
| **5. Approval Level** | Standard | Standard | Destructive only | Minimal | Autonomous | Autonomous |
| **6. Filesystem Scope** | Workspace | Workspace | Workspace | Workspace | Workspace | Workspace |
| **7. Shell Execution** | Approved | Approved | All Workspace | Workspace Cmds | Broad Sandboxed| Unrestricted SB |
| **8. Network Access** | Approved | Approved | Unrestricted | Approved | Unrestricted | Unrestricted |
| **9. Subagents** | Limited | Limited | Enabled | Enabled | Full | Full |
| **10. Background Tasks** | Off | Off | On | On | On | On |
| **11. Memory Persistence**| Session | Session | Persistent | Persistent | Persistent | Persistent |
| **12. Audit Logging** | Always On | Always On | Always On | Always On | Always On | Always On |
| **13. Max Steps** | 25 | 25 | 50 | 100 | 100 | 200 |
| **14. Max Tool Calls** | 50 | 50 | 100 | 200 | 200 | 500 |
| **15. Explicit Deny Rules**| Enforced | Enforced | Enforced | Enforced | Enforced | Enforced |
| **16. Secret Redaction** | Enforced | Enforced | Enforced | Enforced | Enforced | Enforced |

---

### Non-Bypassable Security Boundaries

No preset, system prompt, or instruction can override these six universal invariants:

1. **Provider Policy Boundary**: Remote model APIs enforce their own provider terms of service. Local presets cannot force remote model servers to produce output they refuse to generate.
2. **Operating System ACLs**: The agent process executes under standard host user privileges. It cannot bypass Windows Access Control Lists or escalate privileges.
3. **Explicit Deny Rules**: Absolute programmatic block against dangerous commands (e.g. `rmdir /s /q C:\`, altering system directories like `C:\Windows`, or accessing host credential stores).
4. **Secret Redaction**: Every API key (`sk-*`), GitHub personal access token (`ghp_*`), JWT bearer token, and private key is automatically replaced with `[REDACTED_API_KEY]` across all prompts, outputs, and audit logs.
5. **Sandbox Confinement**: Filesystem tools strictly reject any path resolving outside the active workspace directory. Symlink dereferencing and traversal (`../`) fail closed.
6. **Audit Invariant**: Every event is permanently written to `.agy/audit.jsonl`. No preset or command can turn off or bypass audit logging.

---

## 6. AI Model Provider Integration

`agy-agent` features an open, model-agnostic provider layer. By implementing the industry-standard OpenAI Chat Completions protocol (`/chat/completions`), it connects seamlessly to any local, self-hosted, cloud-hosted, or proxy AI endpoint.

```
                          +------------------------+
                          |       agy-agent        |
                          +-----------+------------+
                                      |
         +----------------------------+----------------------------+
         |                            |                            |
+--------v---------+         +--------v---------+         +--------v---------+
| Local & Private  |         | Commercial Cloud |         | Enterprise Proxy |
+------------------+         +------------------+         +------------------+
| • Ollama         |         | • OpenAI         |         | • LiteLLM Proxy  |
| • vLLM           |         | • OpenRouter     |         | • Virtual Keys   |
| • LM Studio      |         | • DeepSeek       |         | • Multi-Model    |
| • LocalAI        |         | • Groq           |         | • Spend Tracking |
| • llama.cpp      |         | • Mistral AI     |         | • Failover Teams |
+------------------+         +------------------+         +------------------+
```

### Universal OpenAI-Compatible Architecture

All providers communicate over HTTP/HTTPS with standard JSON payloads:
- **Streaming & Non-Streaming Completions**
- **Structured Tool Calling (`tool_calls` schema)**
- **System Instructions & Multi-Turn Message Arrays**
- **Automatic High-Entropy Secret Masking (`sk-...` -> `[REDACTED_API_KEY]`)**

---

### Local & Private AI (Ollama, vLLM, LM Studio)

Run 100% offline, air-gapped coding sessions with zero telemetry and zero external API fees.

#### 1. Ollama (Recommended for Local Workstations)
Install Ollama and pull your preferred coding model:
```bash
# Pull high-performance coding models
ollama run qwen2.5-coder:32b
# or: ollama run deepseek-coder-v2
# or: ollama run llama3.3:70b
```

**Configure in `.agy/config.json`:**
```json
{
  "default_preset": "Unrestricted",
  "provider": {
    "type": "openai",
    "base_url": "http://localhost:11434/v1",
    "api_key": "ollama",
    "model": "qwen2.5-coder:32b",
    "timeout_seconds": 120
  }
}
```

**Or launch on the fly using Environment Variables:**
```bash
# Linux / macOS
export OPENAI_BASE_URL="http://localhost:11434/v1"
export OPENAI_API_KEY="ollama"
export AGY_MODEL="qwen2.5-coder:32b"
agy-agent chat

# Windows PowerShell
$env:OPENAI_BASE_URL = "http://localhost:11434/v1"
$env:OPENAI_API_KEY = "ollama"
$env:AGY_MODEL = "qwen2.5-coder:32b"
agy-agent chat
```

#### 2. vLLM (High-Throughput Multi-GPU Servers)
Launch the vLLM OpenAI-compatible server:
```bash
python -m vllm.entrypoints.openai.api_server \
  --model Qwen/Qwen2.5-Coder-32B-Instruct \
  --port 8000 \
  --api-key secret-vllm-key
```

**Configure in `.agy/config.json`:**
```json
{
  "default_preset": "Aggressive",
  "provider": {
    "type": "openai",
    "base_url": "http://localhost:8000/v1",
    "api_key": "secret-vllm-key",
    "model": "Qwen/Qwen2.5-Coder-32B-Instruct",
    "timeout_seconds": 90
  }
}
```

#### 3. LM Studio / LocalAI
Start local inference in LM Studio and enable the Local Server (port 1234):
```json
{
  "default_preset": "Uncensored",
  "provider": {
    "type": "openai",
    "base_url": "http://localhost:1234/v1",
    "api_key": "lm-studio",
    "model": "local-model",
    "timeout_seconds": 90
  }
}
```

---

### Commercial Cloud AI (OpenAI, OpenRouter, DeepSeek, Groq, Mistral)

Connect to frontier cloud models with high reasoning power.

#### 1. OpenRouter (Unified Multi-Model Gateway)
Access Anthropic Claude 3.5 Sonnet, DeepSeek R1, OpenAI o1, Meta Llama 3.3, and Google Gemini with a single API key:

**Configure in `.agy/config.json`:**
```json
{
  "default_preset": "Aggressive",
  "provider": {
    "type": "openai",
    "base_url": "https://openrouter.ai/api/v1",
    "api_key": "sk-or-v1-your-key-here",
    "model": "anthropic/claude-3.5-sonnet",
    "timeout_seconds": 60
  }
}
```
*Popular OpenRouter models:*
- `anthropic/claude-3.5-sonnet` (Frontier coding and architectural planning)
- `deepseek/deepseek-r1` (Open-weights reasoning)
- `meta-llama/llama-3.3-70b-instruct` (Fast open-weights instruction following)
- `google/gemini-2.0-flash-exp:free` (High-speed multimodal)

#### 2. OpenAI (Direct)
**Configure in `.agy/config.json`:**
```json
{
  "default_preset": "Aggressive",
  "provider": {
    "type": "openai",
    "base_url": "https://api.openai.com/v1",
    "api_key": "sk-proj-your-openai-key",
    "model": "gpt-4o",
    "timeout_seconds": 60
  }
}
```
*Supported OpenAI models:* `gpt-4o`, `gpt-4o-mini`, `o1`, `o1-mini`, `o3-mini`.

#### 3. DeepSeek API (Direct)
Direct high-performance inference at low API cost:
```json
{
  "default_preset": "Aggressive",
  "provider": {
    "type": "openai",
    "base_url": "https://api.deepseek.com/v1",
    "api_key": "sk-your-deepseek-key",
    "model": "deepseek-chat",
    "timeout_seconds": 90
  }
}
```
*Models:* `deepseek-chat` (DeepSeek-V3), `deepseek-reasoner` (DeepSeek-R1).

#### 4. Groq (Ultra-Low Latency Inference)
Harness Groq LPU hardware for near-instant responses:
```json
{
  "default_preset": "Aggressive",
  "provider": {
    "type": "openai",
    "base_url": "https://api.groq.com/openai/v1",
    "api_key": "gsk_your-groq-key",
    "model": "llama-3.3-70b-versatile",
    "timeout_seconds": 30
  }
}
```

#### 5. Mistral AI
High-efficiency European frontier models:
```json
{
  "default_preset": "Aggressive",
  "provider": {
    "type": "openai",
    "base_url": "https://api.mistral.ai/v1",
    "api_key": "your-mistral-api-key",
    "model": "codestral-latest",
    "timeout_seconds": 60
  }
}
```

---

### Enterprise Gateways (LiteLLM Proxy)

In enterprise setups, route requests through a central [LiteLLM Proxy](https://docs.litellm.ai/) for team spend tracking, rate-limiting, and automatic fallback:

```json
{
  "default_preset": "Unrestricted",
  "provider": {
    "type": "openai",
    "base_url": "http://litellm-proxy.corp.internal:4000/v1",
    "api_key": "sk-team-marketing-budget-key",
    "model": "claude-3-5-sonnet",
    "timeout_seconds": 60
  }
}
```

---

### Configuration File (`.agy/config.json`)

`agy-agent` automatically loads `.agy/config.json` from the current workspace root.

```json
{
  "version": 1,
  "default_preset": "Aggressive",
  "workspace_root": ".",
  "auto_approve": false,
  "provider": {
    "type": "openai",
    "base_url": "https://api.openai.com/v1",
    "api_key": "sk-your-key-here",
    "model": "gpt-4o",
    "temperature": 0.2,
    "max_tokens": 4096,
    "timeout_seconds": 60
  },
  "rate_limiting": {
    "max_requests_per_minute": 60,
    "max_tokens_per_minute": 150000
  },
  "audit": {
    "enabled": true,
    "log_path": ".agy/audit.jsonl",
    "redact_secrets": true
  }
}
```

Validate your configuration at any time:
```bash
agy-agent config validate
```

---

### Environment Variables & CLI Overrides

Configuration parameters can be overridden per session without editing files:

| Environment Variable | CLI Flag Equivalent | Description | Default |
| :--- | :--- | :--- | :--- |
| `OPENAI_API_KEY` | *(via prompt)* | API authentication token | `""` (mock fallback) |
| `OPENAI_BASE_URL` | `--base-url <url>` | Endpoint base URL | `https://api.openai.com/v1` |
| `AGY_MODEL` | `--model <model>` | Model identifier | `gpt-4o` |
| `AGY_PRESET` | `--preset <name>` | Behavior & permission preset | `Aggressive` |
| `AGY_WORKSPACE` | `--workspace <path>`| Active workspace root directory | `.` |
| `AGY_CONFIG_PATH` | `--config <file>` | Path to master configuration file | `.agy/config.json` |

#### Quick Examples:
```bash
# Run one task using local Ollama model without changing config file:
OPENAI_BASE_URL="http://localhost:11434/v1" OPENAI_API_KEY="ollama" agy-agent run "Refactor error handling" --model qwen2.5-coder:32b

# Run interactive chat using DeepSeek via OpenRouter under Unleashed preset:
OPENAI_BASE_URL="https://openrouter.ai/api/v1" OPENAI_API_KEY="sk-or-..." agy-agent chat --model deepseek/deepseek-r1 --preset Unleashed
```

---

### Preset & Model Alignment Guide

Understanding how presets interact with different AI models is essential for security and productivity:

| Combination | Effective Experience | Recommended Use Case |
| :--- | :--- | :--- |
| **Local Uncensored Model** (e.g. `qwen2.5-coder` on Ollama) + `Uncensored` or `Unrestricted` Preset | **Zero-Friction Autonomy**: No client-side refusal layer, no server-side refusal layer. Fast, unrestricted local refactoring inside your workspace. | Private offline codebases, security auditing, internal legacy transformations. |
| **Cloud Frontier Model** (e.g. `claude-3.5-sonnet` or `gpt-4o`) + `Aggressive` Preset | **High-Reasoning Production Engineering**: Model performs deep multi-step planning, automated retries, and comprehensive unit tests within safe boundaries. | Production code refactoring, complex feature development, CI/CD pipeline verification. |
| **Reasoning Model** (e.g. `deepseek-r1` or `o1`) + `Heretic` Preset | **Direct Technical Architect**: Model outputs pure technical logic and architectural tradeoffs with minimal conversational pleasantries. | Code reviews, formal verification, algorithm design. |

---

### Persistent Memory (`.agy/memory.json`)

The agent maintains persistent factual memory across sessions in `.agy/memory.json`. Key architectural facts, user preferences, and project rules are preserved during context window compaction.

### Tamper-Evident Audit Log (`.agy/audit.jsonl`)

Every action taken by the agent is recorded in append-only JSON Lines format:
```json
{"timestamp":"2026-11-12T13:40:11Z","preset":"Aggressive","event":"tool_invocation","details":{"tool":"file_read","args":{"path":"Cargo.toml"}}}
{"timestamp":"2026-11-12T13:40:11Z","preset":"Aggressive","event":"tool_completed","details":{"tool":"file_read","success":true}}
```

---

## 7. 104-Rule Engine & Synthetic Oracle Parity

### The 104-Rule Inventory

`agy-patcher` bundles all 104 rules categorized across 9 distinct functional groups:

| Category | Count | Report Type | Description |
|---|---|---|---|
| `harm_categories` | 7 | `enum` | Harm classification label remapping |
| `phish_filters` | 4 | `enum` | Phishing threshold text replacements |
| `brain_filters` | 2 | `enum` | Brain strategy filter label remapping |
| `supercomplete_filters` | 4 | `enum` | Supercomplete filter keyword normalization |
| `instruction_overrides` | 13 | `override` | System instruction mode override tokens |
| `semantic_blocks` | 40 | `semantic` | Safety and semantic text block rewrites |
| `telemetry_functions` | 22 | `telemetry` | Telemetry handler function prefix renames |
| `analytics_endpoints` | 8 | `analytics` | Analytics collection endpoint substitutions |
| `consent_gates` | 4 | `consent` | Consent gate flag remappings |
| **Total** | **104** | **6 report types** | |

### Telemetry No-Op Rules

Of the 22 telemetry function rules, exactly **8 produce identical replacements** (search string == replacement string). `agy-patcher` accounts for these explicitly as `noop_events`, recording the match without mutating the underlying bytes.

### Static Shadow Conflicts

Due to sequential scanning order, 4 rules create static substring collisions:
1. `allow_agent_access_gitignore_files` shadowed within `_allow_agent_access_gitignore_files`
2. `RESTRICTED` shadowed within `ESSENTIAL_USE_CONSENT_STATE_UNRESTRICTED`
3. `RecordFullTrajectoryAnalytics` shadowed within `RecordFullTrajectoryAnalyticsRequest`
4. `RecordGitTelemetry` shadowed within `BatchRecordGitTelemetry`

`agy-patcher plan` flags these conflicts with explicit warnings, and compatibility mode reproduces the exact sequential behavior of the reference oracle.

### Appendix D Oracle Parity Verification

In acceptance test `t09_synthetic_all_rule_oracle_parity`, a synthetic binary fixture containing all 104 targets separated by null bytes was evaluated:

- **Synthetic Fixture SHA-256**: `6da67d7db743e5f560a7dc02f2e02cd87b843ce7538bbbbf401c68a58d3f054b`
- **Expected Output SHA-256**: `e72031619b9947630c3f9784afa7f638a1426d99e622600997796f12a7b282f1`
- **Result**: **Exact Byte Match**. 104 events recorded, 8 telemetry no-ops accounted for, zero length drift.

---

## 8. Building and Testing

### Prerequisites

- **Rust Toolchain**: `stable-x86_64-pc-windows-gnu` or `stable-x86_64-pc-windows-msvc` (Rust 1.80+)
- **MinGW-w64 GCC** (for GNU target): Installed at `C:\mingw64` or on system `PATH`.

```powershell
# Configure PATH for active session (Windows PowerShell)
$env:PATH = "C:\mingw64\bin;C:\Users\MEHULK~1\.cargo\bin;" + $env:PATH
```

### Running Workspace Tests

Run the complete 33-test acceptance and unit test matrix:

```bash
cargo test --workspace --release
```

All 33 tests across all 9 crates will pass:
```
test result: ok. 14 passed (patcher acceptance tests: T01, T02, T03, T06, T08, T09, T10, T12, T13, T14, T18, T20, T23, T24)
test result: ok. 9 passed  (agent acceptance tests: T33, T34, T36, T37, T41, T42, T43, T44, T46)
test result: ok. 10 passed (unit tests across common, policy, tools, patcher-core)
```

### Compiling Production Binaries

#### Local Windows Build:
```bash
cargo build --release --workspace
```
Executables are placed in `target/release/` and distributed via `dist/windows-x64/`:
- `agy-patcher.exe` (10.3 MB)
- `agy-agent.exe` (10.2 MB)

#### Portable Static Linux (MUSL) Build:
Built inside an isolated Docker container with zero glibc host dependencies:
```bash
docker run --rm -v "$PWD:/src" -w /src rust:alpine sh -c "apk add gcc musl-dev && cargo build --release --workspace"
```
Executables are distributed via `dist/linux-x64/` (and 32-bit via `dist/linux-x86/`):
- `agy-patcher` (~6.1 MB)
- `agy-agent` (~6.3 MB)

### Pre-Compiled Distribution Inventory

| Architecture | Platform | Linkage | Binaries | Location |
| :--- | :--- | :--- | :--- | :--- |
| **x86_64** (64-bit) | Windows 10/11 / Server | MinGW-w64 | `agy-patcher.exe`, `agy-agent.exe` | [`dist/windows-x64/`](dist/windows-x64/) |
| **x86_64** (64-bit) | Linux (All distros) | **Static MUSL** | `agy-patcher`, `agy-agent` | [`dist/linux-x64/`](dist/linux-x64/) |
| **i686 / x86** (32-bit) | Linux (Legacy / Embedded) | **Static MUSL** | `agy-patcher`, `agy-agent` | [`dist/linux-x86/`](dist/linux-x86/) |

> [!NOTE]
> Detailed user guides, administration policies, and SIEM integration instructions are available in the **[Admin & User Guide](docs/ADMIN_USER_GUIDE.md)**.

---

## 9. Troubleshooting & FAQ

### Q: Why does building `ring` fail with `failed to find tool gcc.exe`?
**A:** When using the GNU target (`stable-x86_64-pc-windows-gnu`), Rust requires a C compiler for assembly routines. Ensure MinGW-w64 is installed at `C:\mingw64` and add `C:\mingw64\bin` to your `PATH`.

### Q: Why does `agy-patcher apply` fail with `Backup not found` during `restore`?
**A:** `restore` requires `<binary>.original` to exist. When using `apply`, specify `--in-place` to automatically generate the verified baseline backup.

### Q: Why does `agy-patcher apply` report `Stale backup reuse prevented`?
**A:** If an existing `<binary>.original` file does not match the SHA-256 of the input binary, `agy-patcher` blocks the operation to prevent accidentally overwriting the original baseline with an incompatible version.

### Q: Why are my API keys showing up as `[REDACTED_API_KEY]`?
**A:** The secret redaction engine actively scans all prompts, command outputs, and audit logs. This is a non-bypassable security boundary designed to prevent accidental leakage of credentials.

### Q: Does using the `Uncensored` or `Unleashed` preset bypass OpenAI/Anthropic safety filters?
**A:** **No.** Presets govern local client behaviors and permissions. They cannot modify remote server-side model weights or API provider safety policies.

---

## 10. Compliance, Legal & Provenance Notice

1. **No Upstream Binary Redistribution**: This repository and software toolkit does not contain or distribute copyrighted third-party executable binaries, proprietary model weights, or reverse-engineered proprietary source code.
2. **Qualification Status**: Until an authorized Windows executable is provided with cryptographic SHA-256 provenance, `agy-patcher` holds the formal status: *"synthetic compatibility verified; AGY runtime qualification pending"*.
3. **No Warranty**: This software is provided under the terms of the MIT License without warranty of any kind.
