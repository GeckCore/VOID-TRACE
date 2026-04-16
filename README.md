# 🔍 AI-Ghost-Hunter

Stylometric AST-based fingerprinting of AI-generated code.  
Analyzes Rust, Python, JavaScript and TypeScript repositories using
tree-sitter parse trees — not text-matching — to compute a per-file
and global **AI-Score**.

```
╔══════════════════════════════════════════════════════════════════════╗
║     AI-GHOST-HUNTER v1.0  ·  Stylometric AST Code Fingerprinter     ║
║     Rust · Python · JavaScript · TypeScript                          ║
╚══════════════════════════════════════════════════════════════════════╝

╔══════════════════════════════════════════════════════╗
║  REPOSITORY AI-SCORE   87.3%  ║
║  ████████████████████████░░░░░░░░░░░░░░░░░░░░░░  ║
║  ◉ DEFINITE AI GENERATION                           ║
╚══════════════════════════════════════════════════════╝

  CORPUS SUMMARY
  ◼ 247 files analyzed  ◼ 41,892 LOC  ◼ 3 skipped
  ◼ Languages:  189 JS  38 TS  12 Python  8 Rust

  FILE                                              AI-SCORE   LANG    LOC   [N]    [C]    [B]    [V]
  ─────────────────────────────────────────────────────────────────────────────────────────────
  src/services/authenticationService.js               94.2%     JS    312   91.0%  88.0%  97.0%  90.0%
  src/api/userController.ts                           91.7%     TS    198   89.0%  82.0%  96.0%  98.0%
  utils/dataProcessor.py                              88.3%     Py    441   85.0%  91.0%  89.0%  88.0%
```

---

## Installation

**Prerequisites:** Rust ≥ 1.70, a C compiler (for tree-sitter grammars), `git`.

```bash
git clone <this repo>
cd ai-ghost-hunter
cargo build --release
# Binary at: target/release/aigh
```

---

## Usage

```bash
# Analyze a GitHub repository (clones to /tmp automatically)
aigh https://github.com/owner/repo

# Analyze a local directory
aigh /path/to/project

# With GitHub token (private repos, higher rate limit)
aigh https://github.com/owner/private-repo --token ghp_xxxxx
# or: export GITHUB_TOKEN=ghp_xxxxx

# Show raw AST stats per file
aigh /path/to/project --verbose

# Show all files, not just top-40
aigh /path/to/project --all

# Machine-readable JSON
aigh /path/to/project --json | jq '.global_ai_score'

# Only analyze files ≥ 500 bytes
aigh /path/to/project --min-size 500

# Force re-clone (bypass cache)
AIGH_REFRESH=1 aigh https://github.com/owner/repo
```

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                        CLI (clap)                            │
│   --target  --token  --verbose  --json  --top  --min-size    │
└────────────────────────┬─────────────────────────────────────┘
                         │
              ┌──────────▼──────────┐
              │   Ingestion Module   │
              │  • GitHub URL?       │
              │    → git clone --    │
              │      depth 1         │
              │  • Local path?       │
              │    → validate & use  │
              └──────────┬──────────┘
                         │ PathBuf
              ┌──────────▼──────────┐
              │   File Discovery     │
              │  • WalkDir           │
              │  • Skip: node_modules│
              │    .git target dist  │
              │  • Filter: .rs .py   │
              │    .js .ts .jsx .tsx  │
              └──────────┬──────────┘
                         │ Vec<PathBuf>
              ┌──────────▼──────────┐         ┌─────────────────────┐
              │    AST Engine        │         │  tree-sitter grammars│
              │  Per file:           │◄────────│  • tree-sitter-rust  │
              │  1. Read source      │         │  • tree-sitter-python│
              │  2. Parser::new()    │         │  • tree-sitter-js    │
              │  3. parse() → Tree   │         └─────────────────────┘
              │  4. visit() DFS:     │
              │     • identifiers    │
              │     • comments       │
              │     • decision nodes │
              └──────────┬──────────┘
                         │ RawStats
              ┌──────────▼──────────┐
              │    Metric Engine     │
              │                      │
              │  [N] metric_naming() │
              │  [C] metric_comments │
              │  [B] metric_format() │
              │  [V] metric_verbose()│
              │                      │
              │  composite_score()   │
              │  weighted_global()   │
              └──────────┬──────────┘
                         │ Vec<FileAnalysis>
              ┌──────────▼──────────┐
              │     Reporter         │
              │  • Terminal (colored)│
              │  • JSON (serde_json) │
              └─────────────────────┘
```

---

## Metric Algorithms

### [N] Naming Entropy — weight 30%

AI language models produce identifiers that are:
- **Systematically prefixed** — `get_`, `set_`, `handle_`, `process_`, `validate_`, `calculate_`, `initialize_`, etc. Humans use these too, but rarely with this frequency across *all* identifiers in a file.
- **Length-uniform** — LLMs settle in a comfort band of 8–18 chars. Human code mixes short throwaway names (`i`, `tmp`, `buf`, `n`) with long context-specific ones.
- **Convention-pure** — AI-generated code in a snake_case project is *always* snake_case; camelCase projects are *always* camelCase, even in one-off helpers. Humans drift.

**Formula:**
```
score = (ai_prefix_ratio × 0.35)
      + (length_uniformity  × 0.25)   ← 1 - normalize(std_dev / 6)
      + (case_purity        × 0.20)   ← dominant_style / total_ids
      + (sweet_spot_ratio   × 0.20)   ← ids in [8,18] chars / total
```

---

### [C] Comment Predictability — weight 25%

LLMs explain *what* code does; humans document *why* it was written this way.

20 regex patterns trained on GPT-4/Claude output, targeting:
- `"This function/method/struct …"` — most common LLM opener
- `"Returns …"` at line start — docstring-style narration
- `"Checks if/whether …"`, `"Ensures that …"` — condition descriptions
- `"Is responsible for …"`, `"Is used to/for …"` — passive voice on code objects
- `"Converts X to Y"`, `"Iterates over …"` — algorithmic narration
- `"Simply returns/gets/creates …"` — the word "simply" is a strong LLM tell

**Formula:**
```
score = (pattern_match_ratio × 0.55)
      + (comment_density      × 0.25)   ← comments / code_lines
      + (avg_word_count_score × 0.20)   ← normalize([4, 20] words/comment)
```

---

### [B] Boilerplate Consistency — weight 25%

AI generators output formatting that even the best human developers can't
sustain over thousands of lines:

- **Indentation std-dev ≤ 2** — machine-perfect depth increments
- **No mixed tabs + spaces** — humans fix this, but old files drift
- **Line-length distribution** — AI produces a narrow bell curve; human code has long outliers (URLs, debug strings, SQL)
- **Trailing whitespace = 0** — AI never leaves trailing whitespace
- **Blank-line interval regularity** — AI inserts blank lines with structural predictability (function boundaries always, never randomly)

**Formula:**
```
score = (indent_std_score     × 0.30)
      + (line_len_std_score    × 0.25)
      + (trailing_ws_score     × 0.20)
      + (blank_line_regularity × 0.25)
      − mixed_indent_penalty  (0.35 if both tabs AND spaces present)
```

---

### [V] Complexity / Verbosity — weight 20%

**McCabe cyclomatic complexity** = `(decision_nodes) + 1`

Decision nodes counted from AST: `if`, `match`/`switch`, `while`, `for`, `loop`, `try/catch`, ternary `?:`.

Empirical baseline from open-source corpora (Linux kernel, CPython, React):
| Source     | LOC / cyclomatic unit |
|------------|-----------------------|
| Human (p50)| 6–12                  |
| AI (p50)   | 18–35                 |

AI over-writes because LLMs optimize for *apparent clarity*: extra error-handling branches, named intermediate variables, guard clauses, and docstrings for every helper.

**Formula:**
```
loc_per_branch = code_lines / (decisions + 1)
verbosity_score = normalize(loc_per_branch, range=[6, 30])
id_density      = normalize(identifiers / code_lines, max=3.0)

score = (verbosity_score × 0.65) + (id_density × 0.35)
```

---

### Composite Score

```
ai_score = (naming_entropy  × 0.30)
         + (comment_predict × 0.25)
         + (boilerplate     × 0.25)
         + (verbosity       × 0.20)
```

**Global score** = LOC-weighted mean across all analyzed files.

---

## Score Thresholds

| Score     | Verdict              |
|-----------|----------------------|
| ≥ 85%     | Definite AI          |
| 70–85%    | High AI probability  |
| 55–70%    | Moderate AI signal   |
| 40–55%    | Ambiguous            |
| 25–40%    | Likely human         |
| < 25%     | Strong human signal  |

---

## Known Limitations

1. **Calibrated on English-language identifiers.** Non-ASCII variable names will reduce scoring accuracy on all four axes.
2. **Grammar coverage is partial for TypeScript.** The JavaScript grammar handles TSX/TS reasonably but misses type-annotation nodes; some TypeScript-specific identifiers may be undercounted.
3. **Human developers who follow strict style guides** (Google style, rustfmt, black) will score higher on [B] than their actual authorship warrants. This is by design — such code *is* more consistent, regardless of author.
4. **Minified or generated code** (Webpack bundles, protobuf output) will produce false positives. Use `--min-size` to raise the floor.
5. **The tool is probabilistic, not forensic.** Scores are evidence, not proof.

---

## JSON Output Schema

```json
{
  "global_ai_score": 0.873,
  "global_verdict": "definite_ai",
  "files_analyzed": 247,
  "files_skipped": 3,
  "files": [
    {
      "path": "src/services/authService.js",
      "language": "JS",
      "ai_score": 0.942,
      "line_count": 312,
      "verdict": "definite_ai",
      "metrics": {
        "naming_entropy": 0.910,
        "comment_predictability": 0.880,
        "boilerplate_consistency": 0.970,
        "complexity_verbosity": 0.900
      },
      "raw": {
        "identifiers": 284,
        "comments": 41,
        "decision_nodes": 18,
        "code_lines": 271,
        "total_lines": 312
      }
    }
  ]
}
```
