import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { writeFileSync, readFileSync } from "node:fs";

const here = dirname(fileURLToPath(import.meta.url));
const engineDir = resolve(here, "..", "resharp-engine");
const exampleFile = resolve(engineDir, "examples", "popular-crates.rs");
const outFile = resolve(here, "popular-bench.md");
const readmeFile = resolve(here, "..", "README.md");
const BEGIN = "<!-- POPULAR-BENCH:BEGIN -->";
const END = "<!-- POPULAR-BENCH:END -->";

const ENGINES = ["resharp", "regex", "fancy-regex", "pcre2"] as const;
type Engine = (typeof ENGINES)[number];
type Group = "scan" | "validate";

type Row = {
    group: Group;
    pattern: string;
    engine: Engine;
    time: string;
    thrpt: string;
    timeSec: number;
    thrptBps: number;
};

function patternByName(): Map<string, string> {
    const src = readFileSync(exampleFile, "utf8");
    const map = new Map<string, string>();
    for (const m of src.matchAll(/\("([^"]+)",\s*r"([^"]*)"/g)) {
        if (!map.has(m[1])) map.set(m[1], m[2]);
    }
    return map;
}

function label(name: string, patterns: Map<string, string>): string {
    const pat = patterns.get(name) ?? name;
    const shown = pat.length > 30 ? pat.slice(0, 30) + "..." : pat;
    return "`" + shown.replace(/\|/g, "\\|") + "`";
}

function run(): string {
    const args = [
        "run", "--quiet", "--release", "--features", "pcre2-bench",
        "--example", "popular-crates", "--", "--bench", "--noplot",
        "--warm-up-time", process.env.WARMUP ?? "0.3",
        "--measurement-time", process.env.MEASURE ?? "1.5",
        "--sample-size", process.env.SAMPLES ?? "30",
    ];
    const r = spawnSync("cargo", args, { cwd: engineDir, encoding: "utf8", maxBuffer: 64 << 20 });
    if (r.status !== 0) {
        process.stderr.write(r.stdout ?? "");
        process.stderr.write(r.stderr ?? "");
        throw new Error(`cargo exited with status ${r.status}`);
    }
    return r.stdout;
}

function timeToSec(value: number, unit: string): number {
    const c = unit[0];
    if (c === "n") return value * 1e-9;
    if (c === "m") return value * 1e-3;
    if (c === "s") return value;
    return value * 1e-6;
}

function thrptToBps(value: number, unit: string): number {
    const c = unit[0];
    if (c === "K") return value * 1024;
    if (c === "M") return value * 1024 * 1024;
    if (c === "G") return value * 1024 * 1024 * 1024;
    return value;
}

const idRe = /\b(scan|validate)\/([^\s/]+)\/([^\s/]+)/;
const midRe = /:\s*\[\s*\S+\s+\S+\s+([\d.]+)\s+(\S+)\s+/;

function fmt(n: string): string {
    return Number(parseFloat(n).toFixed(2)).toString();
}

function parse(out: string): Row[] {
    const byKey = new Map<string, Row>();
    let cur: { group: Group; pattern: string; engine: Engine } | null = null;
    for (const line of out.split("\n")) {
        const id = line.match(idRe);
        if (id && (ENGINES as readonly string[]).includes(id[3])) {
            cur = { group: id[1] as Group, pattern: id[2], engine: id[3] as Engine };
        }
        if (!cur) continue;
        const key = `${cur.group}/${cur.pattern}/${cur.engine}`;
        const get = (): Row =>
            byKey.get(key) ?? { ...cur!, time: "", thrpt: "", timeSec: 0, thrptBps: 0 };
        if (line.includes("time:")) {
            const m = line.match(midRe);
            if (m) {
                const row = get();
                row.time = `${fmt(m[1])} ${m[2]}`;
                row.timeSec = timeToSec(parseFloat(m[1]), m[2]);
                byKey.set(key, row);
            }
        }
        if (line.includes("thrpt:")) {
            const m = line.match(midRe);
            if (m) {
                const row = get();
                row.thrpt = `${fmt(m[1])} ${m[2]}`;
                row.thrptBps = thrptToBps(parseFloat(m[1]), m[2]);
                byKey.set(key, row);
            }
        }
    }
    return [...byKey.values()];
}

function speedOf(r: Row, metric: "thrpt" | "time"): number {
    return metric === "thrpt" ? r.thrptBps : 1 / r.timeSec;
}

function cell(rows: Row[], group: Group, pattern: string, engine: Engine, metric: "thrpt" | "time"): string {
    const r = rows.find((x) => x.group === group && x.pattern === pattern && x.engine === engine);
    if (!r) return "--";
    const peers = rows.filter((x) => x.group === group && x.pattern === pattern);
    const best = Math.max(...peers.map((x) => speedOf(x, metric)));
    const speed = speedOf(r, metric);
    let ratio = "";
    if (isFinite(best) && best > 0 && speed > 0) {
        ratio = ` (${(best / speed).toFixed(2)}x)`;
    }
    const text = `${r[metric]}${ratio}`;
    return speed >= best ? `**${text}**` : text;
}

function table(rows: Row[], group: Group, metric: "thrpt" | "time", patterns: Map<string, string>): string {
    const pats = [...new Set(rows.filter((r) => r.group === group).map((r) => r.pattern))];
    const header = `| Pattern | ${ENGINES.join(" | ")} |`;
    const sep = `|${"---|".repeat(ENGINES.length + 1)}`;
    const body = pats.map(
        (p) => `| ${label(p, patterns)} | ${ENGINES.map((e) => cell(rows, group, p, e, metric)).join(" | ")} |`,
    );
    return [header, sep, ...body].join("\n");
}

const patterns = patternByName();
const rows = parse(run());
if (rows.length === 0) throw new Error("no benchmark rows parsed");

const md = [
    "resharp uses `UnicodeMode::Full` and `multiline(false)` for apples-to-apples semantics with `regex`, `fancy-regex`, and PCRE2. Ratios in parentheses are relative to the fastest engine per row (1.00x = fastest; higher = that many times slower).",
    "",
    "### Scan (find_all over a 1 MiB haystack), throughput",
    "",
    table(rows, "scan", "thrpt", patterns),
    "",
    "### Validate (is_match on a single value), latency",
    "",
    table(rows, "validate", "time", patterns),
    "",
].join("\n");

writeFileSync(outFile, md);

const readme = readFileSync(readmeFile, "utf8");
const b = readme.indexOf(BEGIN);
const e = readme.indexOf(END);
if (b !== -1 && e !== -1 && e > b) {
    const next = readme.slice(0, b + BEGIN.length) + "\n" + md + "\n" + readme.slice(e);
    writeFileSync(readmeFile, next);
    process.stderr.write(`spliced into ${readmeFile}\n`);
} else {
    process.stderr.write(`markers ${BEGIN} / ${END} not found in README; wrote ${outFile} only\n`);
}

process.stdout.write(md + "\n");
