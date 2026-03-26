import { readdirSync, readFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

type Candidate = {
  path: string;
  expiresAt: string | null;
  expiresAtMs: number;
  scope: string | null;
  rawToken: string;
  variants: Array<{ label: string; token: string }>;
  preview: string;
  score: number;
};

const localAppData = process.env.LOCALAPPDATA;

if (!localAppData) {
  console.error("LOCALAPPDATA is not set.");
  process.exit(1);
}

const args = new Set(process.argv.slice(2));
const top = parseNumberFlag("--top", 8);
const testCount = parseNumberFlag("--test", 0);
const delayMs = parseNumberFlag("--delay", 1200);
const listOnly = args.has("--list") || testCount <= 0;

const cacheDirs = [
  join(
    localAppData,
    "Packages",
    "Microsoft.XboxIdentityProvider_8wekyb3d8bbwe",
    "AC",
    "TokenBroker",
    "Cache",
  ),
  join(
    localAppData,
    "Packages",
    "Microsoft.GamingApp_8wekyb3d8bbwe",
    "AC",
    "TokenBroker",
    "Cache",
  ),
  join(localAppData, "Microsoft", "TokenBroker", "Cache"),
];

function parseNumberFlag(flag: string, fallback: number): number {
  const index = process.argv.indexOf(flag);
  if (index < 0) return fallback;
  const value = Number(process.argv[index + 1]);
  return Number.isFinite(value) ? value : fallback;
}

function parseTokenbrokerFiletime(value: string | null | undefined): number {
  if (!value) return 0;
  try {
    const bytes = Buffer.from(value, "base64");
    if (bytes.length < 8) return 0;
    const filetime = Number(bytes.readBigInt64LE(0));
    if (filetime <= 0) return 0;
    const unixSeconds = Math.floor(filetime / 10_000_000) - 11_644_473_600;
    return unixSeconds * 1000;
  } catch {
    return 0;
  }
}

function scoreCandidate(path: string, scope: string | null): number {
  const lowerPath = path.toLowerCase();
  const lowerScope = (scope ?? "").toLowerCase();
  let score = 0;
  if (lowerPath.includes("xboxidentityprovider")) score += 400;
  if (lowerPath.includes("gamingapp")) score += 250;
  if (lowerScope.includes("xbox") || lowerScope.includes("sisu")) score += 500;
  if (lowerScope.includes("ssl.live.com")) score += 120;
  if (lowerScope.includes("passport.net")) score += 60;
  return score;
}

function previewToken(token: string): string {
  return `${token.slice(0, 12)}...len=${token.length}`;
}

function extractMarkerValue(payload: string, marker: string): string | null {
  const start = payload.indexOf(marker);
  if (start < 0) return null;
  const tail = payload.slice(start + marker.length);
  let seenContent = false;
  let value = "";

  for (const character of tail) {
    if (!seenContent) {
      if (/\s/.test(character) || character === "\0" || isControl(character)) {
        continue;
      }
      seenContent = true;
    }

    if (/\s/.test(character) || character === "\0" || isControl(character)) {
      break;
    }

    value += character;
  }

  return value || null;
}

function isControl(character: string): boolean {
  const code = character.charCodeAt(0);
  return (code >= 0 && code <= 31) || code === 127;
}

function buildVariants(rawToken: string): Array<{ label: string; token: string }> {
  const variants: Array<{ label: string; token: string }> = [];
  const seen = new Set<string>();
  const cleaned = rawToken.trim().replace(/\0/g, "");

  const push = (label: string, token: string) => {
    if (!token || seen.has(token)) return;
    seen.add(token);
    variants.push({ label, token });
  };

  push("raw", cleaned);

  const tIndex = cleaned.indexOf("t=");
  if (tIndex >= 0) {
    const fromT = cleaned.slice(tIndex);
    push("from-t-marker", fromT);
    if (fromT.startsWith("t=")) {
      push("from-t-marker-no-prefix", fromT.slice(2));
    }
    const ampersandIndex = fromT.indexOf("&");
    if (ampersandIndex >= 0) {
      const throughAmpersand = fromT.slice(0, ampersandIndex);
      push("through-ampersand", throughAmpersand);
      if (throughAmpersand.startsWith("t=")) {
        push("through-ampersand-no-prefix", throughAmpersand.slice(2));
      }
    }
  }

  return variants;
}

function decodeDpapiBase64(base64Value: string): string | null {
  const escaped = base64Value.replace(/'/g, "''");
  const script = [
    "Add-Type -AssemblyName System.Security",
    `$enc = [Convert]::FromBase64String('${escaped}')`,
    "$dec = [System.Security.Cryptography.ProtectedData]::Unprotect($enc, $null, [System.Security.Cryptography.DataProtectionScope]::CurrentUser)",
    "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8",
    "[Console]::Write([System.Text.Encoding]::UTF8.GetString($dec))",
  ].join("; ");

  const result = spawnSync("powershell", ["-NoProfile", "-Command", script], {
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 10 * 1024 * 1024,
  });

  if (result.status !== 0) {
    return null;
  }

  return result.stdout;
}

function readCandidate(path: string): Candidate | null {
  try {
    const text = readFileSync(path, "utf16le").replace(/\0+$/g, "");
    const parsed = JSON.parse(text);
    const objectData = parsed?.TBDataStoreObject?.ObjectData;
    const system = objectData?.SystemDefinedProperties;
    const responseBytes = system?.ResponseBytes?.Value;
    if (typeof responseBytes !== "string" || !responseBytes) return null;

    const payload = decodeDpapiBase64(responseBytes);
    if (!payload) return null;

    const rawToken =
      extractMarkerValue(payload, "WTRes_Token") ??
      extractMarkerValue(payload, "t=");
    if (!rawToken) return null;

    const variants = buildVariants(rawToken);
    if (variants.length === 0) return null;

    const expiresAtMs = parseTokenbrokerFiletime(system?.Expiration?.Value);
    if (!expiresAtMs || expiresAtMs <= Date.now()) return null;

    const scope =
      extractMarkerValue(payload, "scope=") ??
      extractMarkerValue(payload, "WA_Scope");

    return {
      path,
      expiresAt: new Date(expiresAtMs).toISOString(),
      expiresAtMs,
      scope,
      rawToken,
      variants,
      preview: previewToken(rawToken),
      score: scoreCandidate(path, scope),
    };
  } catch {
    return null;
  }
}

function collectCandidates(): Candidate[] {
  const results: Candidate[] = [];
  const seen = new Set<string>();

  for (const dir of cacheDirs) {
    if (!existsSync(dir)) continue;
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (!entry.isFile() || !entry.name.endsWith(".tbres")) continue;
      const path = join(dir, entry.name);
      const candidate = readCandidate(path);
      if (!candidate) continue;
      if (seen.has(candidate.rawToken)) continue;
      seen.add(candidate.rawToken);
      results.push(candidate);
    }
  }

  results.sort((left, right) => {
    if (left.expiresAtMs !== right.expiresAtMs) {
      return right.expiresAtMs - left.expiresAtMs;
    }
    if (left.score !== right.score) {
      return right.score - left.score;
    }
    return left.path.localeCompare(right.path);
  });

  return results;
}

async function tryLauncherLogin(token: string) {
  const response = await fetch("https://api.minecraftservices.com/launcher/login", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "User-Agent": "vanillalauncher-bun-probe/0.1.0",
    },
    body: JSON.stringify({
      platform: "ONESTORE",
      xtoken: token,
    }),
  });
  const body = await response.text();
  return {
    status: response.status,
    body: body.slice(0, 200),
  };
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

const candidates = collectCandidates();

console.log(`Found ${candidates.length} non-expired token candidates.`);
for (const [index, candidate] of candidates.slice(0, top).entries()) {
  console.log(
    [
      `#${index + 1}`,
      `exp=${candidate.expiresAt}`,
      `score=${candidate.score}`,
      `scope=${candidate.scope ?? "unknown"}`,
      `preview=${candidate.preview}`,
      `path=${candidate.path}`,
    ].join(" "),
  );
}

if (listOnly) {
  process.exit(0);
}

let remaining = testCount;
for (const candidate of candidates) {
  for (const variant of candidate.variants) {
    if (remaining <= 0) {
      process.exit(0);
    }
    const result = await tryLauncherLogin(variant.token);
    console.log(
      [
        `TEST`,
        `status=${result.status}`,
        `variant=${variant.label}`,
        `exp=${candidate.expiresAt}`,
        `scope=${candidate.scope ?? "unknown"}`,
        `preview=${previewToken(variant.token)}`,
        `path=${candidate.path}`,
        `body=${JSON.stringify(result.body)}`,
      ].join(" "),
    );
    remaining -= 1;
    if (result.status === 429 && remaining > 0) {
      console.log(`Rate limited; waiting ${delayMs}ms before next attempt.`);
    }
    if (remaining > 0) {
      await sleep(delayMs);
    }
  }
}
