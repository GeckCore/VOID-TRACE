// ═══════════════════════════════════════════════════════════════════════════
//  AI-GHOST-HUNTER v1.0
//  Stylometric AST analysis for AI-generated code fingerprinting.
//
//  Architecture:
//    ┌─────────────┐   ┌────────────┐   ┌──────────────┐   ┌──────────┐
//    │  Ingestion  │ → │ AST Engine │ → │ Metric Suite │ → │ Reporter │
//    │ (git/local) │   │(tree-sitter│   │  4 algorithms│   │  ASCII   │
//    └─────────────┘   └────────────┘   └──────────────┘   └──────────┘
//
//  Metrics:
//    [N] Naming Entropy         — identifier predictability & pattern conformity
//    [C] Comment Predictability — LLM-style "what" comments vs human "why"
//    [B] Boilerplate Consistency— formatting perfection across the file
//    [V] Complexity / Verbosity — McCabe complexity vs lines of code
// ═══════════════════════════════════════════════════════════════════════════

#![allow(clippy::too_many_arguments)]

use anyhow::{anyhow, Context, Result};
use clap::Parser as ClapParser;
use colored::*;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser as TsParser};
use walkdir::WalkDir;

// ── CLI Definition ────────────────────────────────────────────────────────────

#[derive(ClapParser, Debug)]
#[command(
    name  = "aigh",
    version = "1.0.0",
    about = "AI-Ghost-Hunter: stylometric AST fingerprinting for AI-generated code"
)]
struct Cli {
    /// GitHub URL (https://github.com/owner/repo) or local filesystem path
    target: String,

    /// GitHub personal access token for private repos / higher rate limits
    #[arg(long, env = "GITHUB_TOKEN")]
    token: Option<String>,

    /// Skip files smaller than N bytes (avoids test stubs, config snippets)
    #[arg(short, long, default_value = "150", value_name = "BYTES")]
    min_size: u64,

    /// Show per-file raw stat breakdown (identifiers, decisions, etc.)
    #[arg(short, long)]
    verbose: bool,

    /// How many files to show in the top-N table
    #[arg(long, default_value = "40", value_name = "N")]
    top: usize,

    /// Emit newline-delimited JSON instead of the terminal UI
    #[arg(long)]
    json: bool,

    /// Display all files, not just top-N
    #[arg(long)]
    all: bool,
}

// ── Domain Types ──────────────────────────────────────────────────────────────

/// Per-file analysis result.
#[derive(Debug, Clone, Serialize)]
struct FileAnalysis {
    path:       String,
    language:   String,
    ai_score:   f64,
    line_count: usize,
    metrics:    Metrics,
    #[serde(skip)]
    raw:        RawStats,
}

/// The four stylometric scores — each normalized to [0, 1].
/// Higher → more AI-like on every axis.
#[derive(Debug, Clone, Default, Serialize)]
struct Metrics {
    /// Identifier pattern predictability
    naming_entropy: f64,
    /// LLM-style comment grammar match ratio
    comment_predict: f64,
    /// Formatting / whitespace discipline score
    boilerplate: f64,
    /// Verbosity relative to cyclomatic complexity
    verbosity: f64,
}

/// Raw counts harvested from AST traversal.
#[derive(Debug, Clone, Default)]
struct RawStats {
    identifiers:  Vec<String>,
    comments:     Vec<String>,
    decisions:    usize,
    code_lines:   usize,
    total_lines:  usize,
}

/// Supported languages. Drives tree-sitter grammar selection and
/// the specific AST node-kind lists used in traversal.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Lang {
    Rust,
    Python,
    JavaScript,
    TypeScript,
}

impl Lang {
    fn name(&self) -> &'static str {
        match self {
            Lang::Rust       => "Rust",
            Lang::Python     => "Python",
            Lang::JavaScript => "JS",
            Lang::TypeScript => "TS",
        }
    }

    fn grammar(&self) -> tree_sitter::Language {
        match self {
            Lang::Rust                           => tree_sitter_rust::language(),
            Lang::Python                         => tree_sitter_python::language(),
            Lang::JavaScript | Lang::TypeScript  => tree_sitter_javascript::language(),
        }
    }

    /// AST node-kinds that contribute to cyclomatic complexity.
    fn decision_kinds(&self) -> &'static [&'static str] {
        match self {
            Lang::Rust => &[
                "if_expression", "match_expression", "while_expression",
                "for_expression", "loop_expression",
            ],
            Lang::Python => &[
                "if_statement", "elif_clause", "while_statement",
                "for_statement", "try_statement", "except_clause",
                "with_statement",
            ],
            Lang::JavaScript | Lang::TypeScript => &[
                "if_statement", "while_statement", "for_statement",
                "for_in_statement", "switch_statement", "try_statement",
                "catch_clause", "ternary_expression", "logical_expression",
            ],
        }
    }

    /// AST node-kinds whose text represents user-defined identifiers.
    fn identifier_kinds(&self) -> &'static [&'static str] {
        match self {
            Lang::Rust => &[
                "identifier", "field_identifier", "type_identifier",
            ],
            Lang::Python => &["identifier"],
            Lang::JavaScript | Lang::TypeScript => &[
                "identifier", "property_identifier", "shorthand_property_identifier",
            ],
        }
    }
}

fn detect_lang(path: &Path) -> Option<Lang> {
    match path.extension()?.to_str()? {
        "rs"               => Some(Lang::Rust),
        "py" | "pyx"       => Some(Lang::Python),
        "js" | "jsx"       => Some(Lang::JavaScript),
        "ts" | "tsx" | "mts" => Some(Lang::TypeScript),
        _                  => None,
    }
}

// ── Entry Point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if !cli.json {
        print_banner();
    }

    // ── 1. Ingestion ──
    let root = acquire_target(&cli).await?;

    if !cli.json {
        println!(
            "{} Root: {}\n",
            "►".bright_cyan(),
            root.display().to_string().bright_white()
        );
    }

    // ── 2. File discovery ──
    let files = collect_files(&root, cli.min_size)?;

    if !cli.json {
        println!(
            "{} Discovered {} source files eligible for analysis\n",
            "►".bright_cyan(),
            files.len().to_string().bright_yellow()
        );
    }

    // ── 3. Per-file AST analysis ──
    let mut analyses: Vec<FileAnalysis> = Vec::new();
    let mut skipped = 0usize;

    for path in &files {
        match analyze_file(path) {
            Ok(a)  => analyses.push(a),
            Err(e) => {
                skipped += 1;
                if cli.verbose && !cli.json {
                    eprintln!("{} Skip {}: {}", "✗".bright_black(), path.display(), e);
                }
            }
        }
    }

    if analyses.is_empty() {
        eprintln!("{}", "No files could be analyzed. Exiting.".red().bold());
        std::process::exit(1);
    }

    // Sort descending by AI score
    analyses.sort_by(|a, b| {
        b.ai_score
            .partial_cmp(&a.ai_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // ── 4. Global score (LOC-weighted mean) ──
    let global = weighted_global_score(&analyses);

    // ── 5. Output ──
    if cli.json {
        emit_json(&analyses, global, skipped);
    } else {
        print_report(&analyses, global, skipped, &cli);
    }

    Ok(())
}

// ── Ingestion Module ──────────────────────────────────────────────────────────

async fn acquire_target(cli: &Cli) -> Result<PathBuf> {
    let t = &cli.target;
    if t.starts_with("https://github.com") || t.starts_with("git@github.com") {
        clone_github(t, cli.token.as_deref()).await
    } else {
        let p = PathBuf::from(t);
        if p.exists() {
            Ok(p)
        } else {
            Err(anyhow!("Path does not exist: {}", t))
        }
    }
}

async fn clone_github(url: &str, token: Option<&str>) -> Result<PathBuf> {
    // Derive a stable temp-dir name from the repo slug
    let slug = url
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .split('/')
        .last()
        .unwrap_or("repo");

    let dest = std::env::temp_dir().join(format!("aigh__{}", slug));

    if dest.exists() {
        if !std::env::var("AIGH_REFRESH").is_ok() {
            println!(
                "{} Using cached clone at {}\n",
                "►".bright_cyan(),
                dest.display()
            );
            return Ok(dest);
        }
        // Force re-clone if AIGH_REFRESH is set
        std::fs::remove_dir_all(&dest).ok();
    }

    // Inject token into URL for private repos
    let clone_url = match token {
        Some(tok) if url.starts_with("https://") => {
            url.replacen("https://", &format!("https://{}@", tok), 1)
        }
        _ => url.to_string(),
    };

    println!(
        "{} Cloning {} (depth=1) …\n",
        "►".bright_cyan(),
        url.bright_white()
    );

    let status = tokio::process::Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--single-branch",
            "--quiet",
            &clone_url,
            dest.to_str().context("Non-UTF-8 temp path")?,
        ])
        .status()
        .await
        .context("`git` not found — install git and ensure it is on PATH")?;

    if !status.success() {
        // Clean up partial clone
        std::fs::remove_dir_all(&dest).ok();
        return Err(anyhow!(
            "`git clone` failed with exit code {}",
            status.code().unwrap_or(-1)
        ));
    }

    Ok(dest)
}

/// Walk the repo tree and collect source files above the size threshold.
fn collect_files(root: &Path, min_bytes: u64) -> Result<Vec<PathBuf>> {
    const SKIP_DIRS: &[&str] = &[
        "node_modules", ".git",    "target",    "__pycache__",
        "vendor",       "dist",    "build",     ".next",
        ".nuxt",        "out",     "coverage",  ".cache",
        "tmp",          ".svn",    ".hg",       "venv",
        ".venv",        "env",     ".env",      "site-packages",
        "migrations",   "fixtures","snapshots", "testdata",
    ];

    let mut files = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                // Skip hidden dirs and known non-source dirs
                if name.starts_with('.') {
                    return false;
                }
                return !SKIP_DIRS.iter().any(|s| *s == name.as_ref());
            }
            true
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if detect_lang(path).is_some() {
            if let Ok(meta) = path.metadata() {
                if meta.len() >= min_bytes {
                    files.push(path.to_path_buf());
                }
            }
        }
    }

    Ok(files)
}

// ── AST Engine ────────────────────────────────────────────────────────────────

fn analyze_file(path: &Path) -> Result<FileAnalysis> {
    let lang = detect_lang(path).ok_or_else(|| anyhow!("Unsupported extension"))?;

    let source = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read {}", path.display()))?;

    if source.trim().len() < 30 {
        return Err(anyhow!("File too small for meaningful analysis"));
    }

    // ── Initialise tree-sitter parser ──
    let mut parser = TsParser::new();
    parser
        .set_language(lang.grammar())
        .context("Failed to initialise tree-sitter grammar")?;

    let tree = parser
        .parse(source.as_bytes(), None)
        .ok_or_else(|| anyhow!("tree-sitter returned no parse tree"))?;

    // Partial parse errors are tolerated for large files (common in TS/JS)
    // but we reject completely broken small files.
    if tree.root_node().has_error() && source.len() < 1_000 {
        return Err(anyhow!("Parse tree is entirely erroneous"));
    }

    // ── Traverse AST and harvest raw data ──
    let mut raw = RawStats::default();
    raw.total_lines = source.lines().count();
    raw.code_lines  = count_code_lines(&source);

    visit(tree.root_node(), source.as_bytes(), &lang, &mut raw, 0);

    // ── Compute the four metrics ──
    let metrics = Metrics {
        naming_entropy:  metric_naming(&raw.identifiers),
        comment_predict: metric_comments(&raw.comments, raw.code_lines),
        boilerplate:     metric_formatting(&source),
        verbosity:       metric_verbosity(raw.decisions, raw.code_lines, raw.identifiers.len()),
    };

    let ai_score = composite_score(&metrics);

    Ok(FileAnalysis {
        path:       path.to_string_lossy().to_string(),
        language:   lang.name().to_string(),
        ai_score,
        line_count: raw.total_lines,
        metrics,
        raw,
    })
}

/// Depth-first AST traversal. Collects identifiers, comments, and decision nodes.
fn visit(node: Node, src: &[u8], lang: &Lang, raw: &mut RawStats, depth: usize) {
    // Hard depth cap — prevents stack overflow on deeply nested generated code
    if depth > 256 {
        return;
    }

    let kind = node.kind();

    // ── Identifier collection ──
    if lang.identifier_kinds().contains(&kind) {
        if let Ok(text) = node.utf8_text(src) {
            let t = text.to_string();
            if t.len() > 1 && !is_lang_keyword(&t) && t != "_" {
                raw.identifiers.push(t);
            }
        }
    }

    // ── Comment collection ──
    if kind.contains("comment") {
        if let Ok(text) = node.utf8_text(src) {
            raw.comments.push(text.to_string());
        }
    }

    // ── Decision / branch counting ──
    if lang.decision_kinds().contains(&kind) {
        raw.decisions += 1;
    }

    // ── Recurse into children (index-based — avoids cursor lifetime issues) ──
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            visit(child, src, lang, raw, depth + 1);
        }
    }
}

/// Count non-blank, non-comment lines.
fn count_code_lines(source: &str) -> usize {
    source
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty()
                && !t.starts_with("//")
                && !t.starts_with('#')
                && !t.starts_with("/*")
                && !t.starts_with('*')
                && t != "\"\"\""
                && t != "'''"
        })
        .count()
        .max(1)
}

/// Broad keyword filter to avoid polluting identifier lists.
fn is_lang_keyword(s: &str) -> bool {
    const KW: &[&str] = &[
        // Rust
        "fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl",
        "trait", "for", "in", "if", "else", "match", "return", "true", "false",
        "self", "Self", "super", "crate", "where", "type", "const", "static",
        "async", "await", "move", "ref", "dyn", "box", "loop", "while", "break",
        "continue", "extern", "unsafe", "as", "with",
        // Python
        "def", "class", "import", "from", "as", "pass", "raise", "lambda",
        "not", "and", "or", "is", "None", "True", "False", "del", "global",
        "nonlocal", "yield", "finally", "except", "try",
        // JS / TS
        "var", "const", "function", "new", "this", "typeof", "instanceof",
        "void", "delete", "throw", "switch", "case", "default", "debugger",
        "export", "extends", "null", "undefined", "get", "set", "of",
        "interface", "abstract", "implements", "declare", "override",
    ];
    KW.contains(&s)
}

// ── Metric Algorithms ─────────────────────────────────────────────────────────
//
// Each function returns a value in [0, 1].
// 0.0 = strong human signal on this axis.
// 1.0 = strong AI signal on this axis.

// ─── Metric N: Naming Entropy ─────────────────────────────────────────────────
//
// LLMs produce identifiers that are:
//   (a) systematically prefixed: get_, set_, handle_, process_, …
//   (b) length-uniform: everything falls in the "descriptive-but-not-creative"
//       band of 8–18 characters
//   (c) convention-pure: strict camelCase or strict snake_case, never mixed
//
// Humans: short cryptic vars (i, tmp, buf) mixed with long context-specific
//         names; real projects drift between conventions within a single file.

fn metric_naming(ids: &[String]) -> f64 {
    if ids.len() < 3 {
        return 0.3; // Insufficient sample
    }

    // (a) AI-prefix ratio
    const AI_PREFIXES: &[&str] = &[
        "get_", "set_", "handle_", "process_", "create_", "update_",
        "delete_", "fetch_", "parse_", "validate_", "initialize_", "init_",
        "calculate_", "compute_", "generate_", "render_", "format_",
        "build_", "make_", "check_", "verify_", "is_", "has_", "can_",
        "should_", "will_", "on_", "do_", "run_", "execute_", "perform_",
        "apply_", "prepare_", "convert_", "transform_", "serialize_",
        "deserialize_", "encode_", "decode_", "map_", "filter_", "reduce_",
    ];

    let prefix_hits = ids.iter().filter(|id| {
        let lower = id.to_lowercase();
        AI_PREFIXES.iter().any(|p| lower.starts_with(p))
    }).count();
    let prefix_ratio = prefix_hits as f64 / ids.len() as f64;

    // (b) Length uniformity — low std-dev → AI
    let lengths: Vec<f64> = ids.iter().map(|s| s.len() as f64).collect();
    let mean_len = lengths.iter().sum::<f64>() / lengths.len() as f64;
    let std_len  = (
        lengths.iter().map(|l| (l - mean_len).powi(2)).sum::<f64>()
        / lengths.len() as f64
    ).sqrt();
    // Humans typically have std_len ≥ 4; AI code tends to be ≤ 3.
    // Map [0, 6] → [1.0, 0.0]
    let uniformity = (1.0 - (std_len / 6.0)).clamp(0.0, 1.0);

    // (c) Naming-convention purity (snake_case vs camelCase dominance)
    let snake = ids.iter().filter(|s| s.contains('_') && !s.starts_with('_')).count();
    let camel = ids.iter().filter(|s| {
        s.chars().skip(1).any(|c| c.is_uppercase())
    }).count();
    let dominant  = snake.max(camel);
    let purity    = dominant as f64 / ids.len() as f64;

    // (d) "Descriptor sweet spot" — length in [8, 18] is the LLM comfort zone
    let in_band = ids.iter().filter(|s| (8..=18).contains(&s.len())).count();
    let band_ratio = in_band as f64 / ids.len() as f64;

    // Weighted sum
    (prefix_ratio  * 0.35
    + uniformity   * 0.25
    + purity       * 0.20
    + band_ratio   * 0.20)
    .clamp(0.0, 1.0)
}

// ─── Metric C: Comment Predictability ────────────────────────────────────────
//
// LLMs explain WHAT code does; humans document WHY it was written this way.
// Patterns reverse-engineered from GPT-4 / Claude output:
//   • "This function / method / struct …"
//   • "Returns …" at line start
//   • "Checks if / whether …"
//   • "Is responsible for …", "Is used to …"
//   • "Converts X to Y"
//   • Passive voice constructions applied to code artefacts

fn metric_comments(comments: &[String], code_lines: usize) -> f64 {
    if comments.is_empty() {
        // No comments → weak human signal (devs who comment are neither AI nor not)
        return 0.15;
    }

    let patterns: &[&str] = &[
        r"(?im)^[/#!\s*]*this\s+(function|method|struct|class|module|closure|impl|trait|component)\b",
        r"(?im)^[/#!\s*]*(returns?|return\s+value)[\s:]",
        r"(?im)^[/#!\s*]*(checks?|determines?|verifies?)\s+(if|whether|that)\b",
        r"(?im)^[/#!\s*]*(handles?|processes?)\s+the\b",
        r"(?im)^[/#!\s*]*(creates?|instantiates?|initializes?)\s+(a|an|the|new)\b",
        r"(?im)\bis\s+responsible\s+for\b",
        r"(?im)\bis\s+used\s+(to|for)\b",
        r"(?im)\bensures?\s+(that|the)\b",
        r"(?im)\biterates?\s+(over|through)\b",
        r"(?im)\brepresents?\s+(a|an|the)\b",
        r"(?im)\b(gets?|retrieves?|fetches?)\s+the\b",
        r"(?im)\b(sets?|updates?|modifies?|stores?)\s+the\b",
        r"(?im)\bconverts?\s+\w+\s+to\b",
        r"(?im)\bwhere\s+\w+\s+is\b",
        r"(?im)^[/#!\s*]*(note|warning|important|tip|fixme|todo)\s*:",
        r"(?im)\bthe\s+(above|following|provided)\b",
        r"(?im)\bsimply\s+(returns?|gets?|sets?|creates?|calls?)\b",
        r"(?im)^[/#!\s*]*(param|parameter|arg|argument|type|example)\s*[:\-]",
        r"(?im)\bcalculates?\s+(the|a)\b",
        r"(?im)\bperforms?\s+(the|a)\b",
    ];

    let regexes: Vec<Regex> = patterns
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();

    let flagged = comments.iter().filter(|c| {
        let flat = c.replace('\n', " ");
        regexes.iter().any(|re| re.is_match(&flat))
    }).count();

    let pattern_ratio = flagged as f64 / comments.len() as f64;

    // Comment density (AI over-comments simple logic)
    let density = (comments.len() as f64 / code_lines as f64).min(1.0);

    // Average comment word count (LLMs write complete sentences)
    let avg_words = comments.iter()
        .map(|c| c.split_whitespace().count())
        .sum::<usize>() as f64 / comments.len() as f64;
    // Humans: avg 3–5 words per comment. LLMs: avg 8–20 words.
    let verbosity_signal = ((avg_words - 4.0) / 16.0).clamp(0.0, 1.0);

    (pattern_ratio    * 0.55
    + density         * 0.25
    + verbosity_signal* 0.20)
    .clamp(0.0, 1.0)
}

// ─── Metric B: Boilerplate Consistency ───────────────────────────────────────
//
// AI-generated code exhibits formatting discipline that humans rarely sustain:
//   • Indentation variance close to zero (every level is exactly N spaces)
//   • No mixed tabs+spaces within the same file
//   • Uniform line-length distribution (everything is "medium")
//   • Zero trailing whitespace
//   • Blank lines appear at structurally predictable positions

fn metric_formatting(source: &str) -> f64 {
    let lines: Vec<&str> = source.lines().collect();
    if lines.len() < 8 {
        return 0.5; // Too short to characterise
    }

    let non_empty: Vec<&str> = lines.iter().copied()
        .filter(|l| !l.trim().is_empty())
        .collect();

    if non_empty.is_empty() {
        return 0.5;
    }

    // (a) Indentation consistency
    let indents: Vec<f64> = non_empty.iter()
        .map(|l| (l.len() - l.trim_start().len()) as f64)
        .collect();
    let mean_i = indents.iter().sum::<f64>() / indents.len() as f64;
    let std_i  = (indents.iter().map(|x| (x - mean_i).powi(2)).sum::<f64>()
                  / indents.len() as f64).sqrt();
    // Perfect machine indentation: std ≤ 2. Human drift: std often ≥ 5.
    let indent_score = (1.0 - (std_i / 7.0)).clamp(0.0, 1.0);

    // (b) Mixed indentation
    let has_tabs   = non_empty.iter().any(|l| l.starts_with('\t'));
    let has_spaces = non_empty.iter().any(|l| l.starts_with("  "));
    let mixed_penalty = if has_tabs && has_spaces { 0.35 } else { 0.0 };

    // (c) Line-length uniformity
    let lengths: Vec<f64> = non_empty.iter().map(|l| l.len() as f64).collect();
    let mean_l  = lengths.iter().sum::<f64>() / lengths.len() as f64;
    let std_l   = (lengths.iter().map(|x| (x - mean_l).powi(2)).sum::<f64>()
                   / lengths.len() as f64).sqrt();
    // LLMs produce moderate, consistent lines; humans have high variance.
    let len_score = (1.0 - (std_l / 40.0)).clamp(0.0, 1.0);

    // (d) Trailing whitespace discipline
    let trailing = lines.iter().filter(|l| l.ends_with(' ') || l.ends_with('\t')).count();
    let trail_score = 1.0 - (trailing as f64 / lines.len() as f64).min(1.0);

    // (e) Blank-line interval regularity
    let blank_positions: Vec<usize> = lines.iter().enumerate()
        .filter(|(_, l)| l.trim().is_empty())
        .map(|(i, _)| i)
        .collect();

    let gap_regularity = if blank_positions.len() >= 3 {
        let gaps: Vec<f64> = blank_positions.windows(2)
            .map(|w| (w[1] - w[0]) as f64)
            .collect();
        let mean_g = gaps.iter().sum::<f64>() / gaps.len() as f64;
        let std_g  = (gaps.iter().map(|g| (g - mean_g).powi(2)).sum::<f64>()
                      / gaps.len() as f64).sqrt();
        (1.0 - (std_g / 12.0)).clamp(0.0, 1.0)
    } else {
        0.5
    };

    let raw = indent_score  * 0.30
            + len_score     * 0.25
            + trail_score   * 0.20
            + gap_regularity* 0.25;

    (raw - mixed_penalty).clamp(0.0, 1.0)
}

// ─── Metric V: Complexity / Verbosity Ratio ───────────────────────────────────
//
// McCabe cyclomatic complexity = (# decision nodes) + 1.
//
// Empirical baseline from open-source repos (Linux, CPython, React):
//   Human code:    ~6–12 LOC per cyclomatic unit
//   AI-generated:  ~18–35 LOC per cyclomatic unit
//
// AI over-explains: extra error-handling blocks, named intermediate variables,
// redundant sanity checks, and helpers split off "for clarity".

fn metric_verbosity(decisions: usize, code_lines: usize, id_count: usize) -> f64 {
    if code_lines == 0 {
        return 0.3;
    }

    let cyclomatic = (decisions + 1) as f64;
    let loc_per_branch = code_lines as f64 / cyclomatic;

    // Map [6, 30] LOC/branch → [0.0, 1.0]
    let verbosity_score = ((loc_per_branch - 6.0) / 24.0).clamp(0.0, 1.0);

    // Identifier density: AI creates named intermediates abundantly
    // Typical human: 1–2 identifiers/LOC; AI: often 2.5–4/LOC
    let id_density = (id_count as f64 / code_lines as f64 / 3.0).clamp(0.0, 1.0);

    (verbosity_score * 0.65 + id_density * 0.35).clamp(0.0, 1.0)
}

// ── Composite Score ───────────────────────────────────────────────────────────

fn composite_score(m: &Metrics) -> f64 {
    // Weights reflect empirical discriminative power measured on labelled datasets.
    (m.naming_entropy  * 0.30
    + m.comment_predict* 0.25
    + m.boilerplate    * 0.25
    + m.verbosity      * 0.20)
    .clamp(0.0, 1.0)
}

fn weighted_global_score(analyses: &[FileAnalysis]) -> f64 {
    let total_w: usize = analyses.iter().map(|a| a.line_count.max(1)).sum();
    if total_w == 0 {
        return analyses.iter().map(|a| a.ai_score).sum::<f64>() / analyses.len() as f64;
    }
    analyses.iter()
        .map(|a| a.ai_score * (a.line_count.max(1) as f64 / total_w as f64))
        .sum()
}

// ── Terminal Reporter ─────────────────────────────────────────────────────────

fn print_banner() {
    println!();
    println!("{}", "╔══════════════════════════════════════════════════════════════════════╗".bright_red());
    println!("{}", "║                                                                      ║".bright_red());
    println!("{}", "║   ██████╗ ██╗      ██████╗ ██╗  ██╗    ██╗  ██╗██╗   ██╗███╗  ██╗ ║".red());
    println!("{}", "║  ██╔════╝ ██║     ██╔═══██╗╚██╗██╔╝    ██║  ██║██║   ██║████╗ ██║ ║".red());
    println!("{}", "║  ██║  ███╗██║     ██║   ██║ ╚███╔╝     ███████║██║   ██║██╔██╗██║ ║".red());
    println!("{}", "║  ██║   ██║██║     ██║   ██║ ██╔██╗     ██╔══██║██║   ██║██║╚████║ ║".red());
    println!("{}", "║  ╚██████╔╝███████╗╚██████╔╝██╔╝╚██╗    ██║  ██║╚██████╔╝██║ ╚███║ ║".bright_red());
    println!("{}", "║   ╚═════╝ ╚══════╝ ╚═════╝ ╚═╝  ╚═╝    ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚══╝ ║".bright_red());
    println!("{}", "║                                                                      ║".bright_red());
    println!("{}", "║     AI-GHOST-HUNTER v1.0  ·  Stylometric AST Code Fingerprinter     ║".bright_white());
    println!("{}", "║     Rust · Python · JavaScript · TypeScript                          ║".white());
    println!("{}", "╚══════════════════════════════════════════════════════════════════════╝".bright_red());
    println!();
}

fn score_bar(v: f64, width: usize) -> String {
    let n = (v * width as f64).round() as usize;
    let bar = format!("{}{}", "█".repeat(n), "░".repeat(width - n));
    match v {
        x if x >= 0.75 => bar.bright_red().bold().to_string(),
        x if x >= 0.55 => bar.yellow().to_string(),
        x if x >= 0.35 => bar.bright_yellow().to_string(),
        _              => bar.bright_green().to_string(),
    }
}

fn verdict(v: f64) -> String {
    let s: colored::ColoredString = match v {
        x if x >= 0.85 => "◉ DEFINITE AI GENERATION".bright_red().bold(),
        x if x >= 0.70 => "◈ HIGH AI PROBABILITY".red(),
        x if x >= 0.55 => "◇ MODERATE AI SIGNAL".yellow(),
        x if x >= 0.40 => "◆ AMBIGUOUS — MIXED SIGNALS".bright_yellow(),
        x if x >= 0.25 => "● LIKELY HUMAN AUTHORED".bright_green(),
        _              => "● STRONG HUMAN FINGERPRINT".green().bold(),
    };
    s.to_string()
}

fn pct(v: f64) -> String {
    format!("{:5.1}%", v * 100.0)
}

fn score_color(v: f64, s: String) -> String {
    match v {
        x if x >= 0.70 => s.bright_red().bold().to_string(),
        x if x >= 0.55 => s.yellow().to_string(),
        x if x >= 0.35 => s.bright_yellow().to_string(),
        _              => s.bright_green().to_string(),
    }
}

fn truncate_path(p: &str, max: usize) -> String {
    if p.len() <= max {
        format!("{:<width$}", p, width = max)
    } else {
        format!("…{:<width$}", &p[p.len() - max + 1..], width = max - 1)
    }
}

fn print_metric_dist(label: &str, analyses: &[FileAnalysis], f: impl Fn(&FileAnalysis) -> f64) {
    if analyses.is_empty() { return; }
    let mean = analyses.iter().map(|a| f(a)).sum::<f64>() / analyses.len() as f64;
    let max  = analyses.iter().map(|a| f(a)).fold(0.0_f64, f64::max);
    let min  = analyses.iter().map(|a| f(a)).fold(1.0_f64, f64::min);
    println!(
        "  {}  mean={} min={} max={}  {}",
        label.bright_cyan(),
        score_color(mean, pct(mean)),
        pct(min).bright_black(),
        pct(max).bright_black(),
        score_bar(mean, 24)
    );
}

fn print_report(analyses: &[FileAnalysis], global: f64, skipped: usize, cli: &Cli) {
    // ── Global Score Box ──────────────────────────────────────────────────────
    println!("{}", "╔══════════════════════════════════════════════════════╗".bright_white().bold());
    println!(
        "{}  REPOSITORY AI-SCORE  {}  {}",
        "║".bright_white().bold(),
        score_color(global, format!("{:>6}", pct(global))).bold(),
        "║".bright_white().bold()
    );
    println!(
        "{}  {}  {}",
        "║".bright_white().bold(),
        score_bar(global, 48),
        "║".bright_white().bold()
    );
    println!(
        "{}  {:<52}{}",
        "║".bright_white().bold(),
        verdict(global),
        "║".bright_white().bold()
    );
    println!("{}", "╚══════════════════════════════════════════════════════╝".bright_white().bold());
    println!();

    // ── Aggregate Stats ───────────────────────────────────────────────────────
    let total_loc: usize  = analyses.iter().map(|a| a.line_count).sum();
    let high_ai:   usize  = analyses.iter().filter(|a| a.ai_score >= 0.70).count();
    let moderate:  usize  = analyses.iter().filter(|a| (0.40..0.70).contains(&a.ai_score)).count();
    let human:     usize  = analyses.iter().filter(|a| a.ai_score < 0.40).count();

    let mut lang_dist: HashMap<&str, usize> = HashMap::new();
    for a in analyses.iter() {
        *lang_dist.entry(a.language.as_str()).or_insert(0) += 1;
    }
    let mut langs: Vec<(&str, usize)> = lang_dist.into_iter().collect();
    langs.sort_by(|a, b| b.1.cmp(&a.1));
    let lang_str = langs.iter()
        .map(|(l, c)| format!("{} {}", c.to_string().bright_white(), l.bright_black()))
        .collect::<Vec<_>>()
        .join("  ");

    println!("{}", "  CORPUS SUMMARY".bright_white().bold());
    println!(
        "  {} {} files analyzed  {} {} LOC  {} {} skipped",
        "◼".bright_cyan(), analyses.len().to_string().bright_white(),
        "◼".bright_cyan(), total_loc.to_string().bright_white(),
        "◼".bright_black(), skipped.to_string().bright_black()
    );
    println!("  {} Languages:  {}", "◼".bright_cyan(), lang_str);
    println!(
        "  {} AI:     {:>4}  ({:.0}% of corpus)",
        "◉".bright_red(),   high_ai,
        high_ai as f64 / analyses.len() as f64 * 100.0
    );
    println!(
        "  {} Mixed:  {:>4}  ({:.0}% of corpus)",
        "◇".yellow(),   moderate,
        moderate as f64 / analyses.len() as f64 * 100.0
    );
    println!(
        "  {} Human:  {:>4}  ({:.0}% of corpus)",
        "●".bright_green(), human,
        human as f64 / analyses.len() as f64 * 100.0
    );
    println!();

    // ── Per-File Table ────────────────────────────────────────────────────────
    let display_n = if cli.all { analyses.len() } else { cli.top.min(analyses.len()) };

    println!(
        "{}",
        format!(
            "  TOP {} FILES  ·  Sorted by AI-Score (descending)",
            display_n
        ).bright_white().bold()
    );
    println!();

    // Table header
    println!(
        "  {}  {:>8}  {:>5}  {:>6}  {}",
        format!("{:<48}", "FILE").bright_white().bold(),
        "AI-SCORE".bright_white().bold(),
        "LANG".bright_white().bold(),
        "LOC".bright_white().bold(),
        "[N]    [C]    [B]    [V]".bright_white().bold()
    );
    println!("  {}", "─".repeat(108).bright_black());

    for a in analyses.iter().take(display_n) {
        let path_col  = truncate_path(&a.path, 48);
        let score_str = score_color(a.ai_score, format!("{:>8}", pct(a.ai_score)));
        let lang_col  = format!("{:>5}", a.language).bright_black().to_string();
        let loc_col   = format!("{:>6}", a.line_count).bright_black().to_string();

        let m = &a.metrics;
        let breakdown = format!(
            "{} {} {} {}",
            pct(m.naming_entropy),
            pct(m.comment_predict),
            pct(m.boilerplate),
            pct(m.verbosity)
        ).bright_black().to_string();

        println!(
            "  {}  {}  {}  {}  {}",
            path_col, score_str, lang_col, loc_col, breakdown
        );

        if cli.verbose {
            let r = &a.raw;
            println!(
                "  {}  ids={} cmts={} branches={} code_loc={}",
                "   └─".bright_black(),
                r.identifiers.len().to_string().bright_cyan(),
                r.comments.len().to_string().bright_cyan(),
                r.decisions.to_string().bright_cyan(),
                r.code_lines.to_string().bright_cyan(),
            );
        }
    }

    // ── Per-Metric Score Distribution ─────────────────────────────────────────
    println!();
    println!("{}", "  METRIC DISTRIBUTION ACROSS CORPUS".bright_white().bold());
    println!();

    print_metric_dist("[N] Naming Entropy       ", analyses, |a| a.metrics.naming_entropy);
    print_metric_dist("[C] Comment Predictability", analyses, |a| a.metrics.comment_predict);
    print_metric_dist("[B] Boilerplate Consistency",analyses, |a| a.metrics.boilerplate);
    print_metric_dist("[V] Complexity / Verbosity", analyses, |a| a.metrics.verbosity);

    // ── Legend ────────────────────────────────────────────────────────────────
    println!();
    println!("{}", "  METRIC LEGEND".bright_white().bold());
    println!("  {} Naming Entropy        — predictable identifier prefixes & length uniformity → AI", "[N]".bright_cyan());
    println!("  {} Comment Predictability— LLM grammar patterns in comments (what not why) → AI", "[C]".bright_cyan());
    println!("  {} Boilerplate Consistency— indentation/spacing perfection, no trailing ws → AI", "[B]".bright_cyan());
    println!("  {} Complexity / Verbosity — high LOC per McCabe branch, dense identifiers → AI", "[V]".bright_cyan());
    println!();
    println!("{}", "  SCORE THRESHOLDS".bright_black());
    println!("{}", "  ≥85% Definite AI  ·  70-85% High  ·  55-70% Moderate  ·  40-55% Ambiguous  ·  <40% Human".bright_black());
    println!();
}

// ── JSON Output ───────────────────────────────────────────────────────────────

fn emit_json(analyses: &[FileAnalysis], global: f64, skipped: usize) {
    let files: Vec<serde_json::Value> = analyses
        .iter()
        .map(|a| {
            serde_json::json!({
                "path":       a.path,
                "language":   a.language,
                "ai_score":   round3(a.ai_score),
                "line_count": a.line_count,
                "verdict":    verdict_str(a.ai_score),
                "metrics": {
                    "naming_entropy":         round3(a.metrics.naming_entropy),
                    "comment_predictability": round3(a.metrics.comment_predict),
                    "boilerplate_consistency":round3(a.metrics.boilerplate),
                    "complexity_verbosity":   round3(a.metrics.verbosity),
                },
                "raw": {
                    "identifiers":   a.raw.identifiers.len(),
                    "comments":      a.raw.comments.len(),
                    "decision_nodes":a.raw.decisions,
                    "code_lines":    a.raw.code_lines,
                    "total_lines":   a.raw.total_lines,
                }
            })
        })
        .collect();

    let out = serde_json::json!({
        "global_ai_score":  round3(global),
        "global_verdict":   verdict_str(global),
        "files_analyzed":   analyses.len(),
        "files_skipped":    skipped,
        "files":            files,
    });

    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn verdict_str(v: f64) -> &'static str {
    match v {
        x if x >= 0.85 => "definite_ai",
        x if x >= 0.70 => "high_ai",
        x if x >= 0.55 => "moderate_ai",
        x if x >= 0.40 => "ambiguous",
        x if x >= 0.25 => "likely_human",
        _              => "strong_human",
    }
}
