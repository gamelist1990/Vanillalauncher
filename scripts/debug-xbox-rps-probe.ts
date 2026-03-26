import { readdirSync, readFileSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";

type TokenCandidate = {
  path: string;
  scope: string | null;
  expiresAtMs: number;
  expiresAt: string;
  variants: Array<{ label: string; ticket: string }>;
};

type UserAuthSuccess = {
  candidate: TokenCandidate;
  label: string;
  ticket: string;
  userToken: string;
  uhs: string | null;
};

type LastSuccessState = {
  sourcePath: string;
  variantLabel: string;
  relyingParty: string;
  ticketPrefix: string;
  expiresAt: string;
  savedAt: string;
};

type Attempt = {
  candidate: TokenCandidate;
  label: string;
  ticket: string;
  score: number;
};

const localAppData = process.env.LOCALAPPDATA;
if (!localAppData) {
  console.error("LOCALAPPDATA is not set.");
  process.exit(1);
}

const args = new Set(process.argv.slice(2));
const maxCandidates = parseNumberFlag("--max-candidates", 35);
const maxAttempts = parseNumberFlag("--max-attempts", 12);
const delayMs = parseNumberFlag("--delay", 450);
const bodyPreview = parseNumberFlag("--body-preview", 240);
const fullSearch = args.has("--full-search");
const allowAltRelyingParties = args.has("--allow-alt-rp");
const noState = args.has("--no-state");

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

const preferredRelyingParty = "rp://api.minecraftservices.com/";
const altRelyingParties = ["http://xboxlive.com", "rp://xboxlive.com/"];

const statePath = join(
  process.env.TEMP ?? join(localAppData, "Temp"),
  "VanillaLauncher",
  "xbox-rps-last-success.json",
);

function parseNumberFlag(flag: string, fallback: number): number {
  const index = process.argv.indexOf(flag);
  if (index < 0) return fallback;
  const parsed = Number(process.argv[index + 1]);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function readLastSuccessState(): LastSuccessState | null {
  if (noState || !existsSync(statePath)) return null;
  try {
    const parsed = JSON.parse(readFileSync(statePath, "utf8")) as LastSuccessState;
    if (!parsed.sourcePath || !parsed.variantLabel) return null;
    return parsed;
  } catch {
    return null;
  }
}

function writeLastSuccessState(state: LastSuccessState) {
  try {
    mkdirSync(dirname(statePath), { recursive: true });
    writeFileSync(statePath, JSON.stringify(state, null, 2), "utf8");
  } catch {
    // best-effort only
  }
}

function preview(value: string): string {
  return `${value.slice(0, 18)}...len=${value.length}`;
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

  return result.status === 0 ? result.stdout : null;
}

function extractMarkerValue(payload: string, marker: string): string | null {
  const start = payload.indexOf(marker);
  if (start < 0) return null;

  const tail = payload.slice(start + marker.length);
  let seen = false;
  let value = "";

  for (const ch of tail) {
    const code = ch.charCodeAt(0);
    const control = code <= 31 || code === 127;

    if (!seen) {
      if (/\s/.test(ch) || ch === "\0" || control) continue;
      seen = true;
    }
    if (/\s/.test(ch) || ch === "\0" || control) break;
    value += ch;
  }

  return value || null;
}

function parseTokenbrokerFiletime(value: string | null | undefined): number {
  if (!value) return 0;
  try {
    const bytes = Buffer.from(value, "base64");
    if (bytes.length < 8) return 0;
    const filetime = Number(bytes.readBigInt64LE(0));
    if (filetime <= 0) return 0;
    return (Math.floor(filetime / 10_000_000) - 11_644_473_600) * 1000;
  } catch {
    return 0;
  }
}

function buildTicketVariants(raw: string): Array<{ label: string; ticket: string }> {
  const cleaned = raw.trim().replace(/\0/g, "");
  if (!cleaned) return [];

  const variants: Array<{ label: string; ticket: string }> = [];
  const seen = new Set<string>();

  const push = (label: string, ticket: string) => {
    if (!ticket || seen.has(ticket)) return;
    seen.add(ticket);
    variants.push({ label, ticket });
  };

  push("raw", cleaned);
  const tIndex = cleaned.indexOf("t=");
  if (tIndex >= 0) {
    const fromT = cleaned.slice(tIndex);
    push("from-t", fromT);
    if (fromT.startsWith("t=")) {
      push("from-t-no-prefix", fromT.slice(2));
    }

    const ampIndex = fromT.indexOf("&");
    if (ampIndex >= 0) {
      const beforeAmp = fromT.slice(0, ampIndex);
      push("through-ampersand", beforeAmp);
      if (beforeAmp.startsWith("t=")) {
        push("through-ampersand-no-prefix", beforeAmp.slice(2));
      }
    }
  }

  return variants;
}

function rankAttempt(
  candidate: TokenCandidate,
  label: string,
  ticket: string,
  lastSuccess: LastSuccessState | null,
): number {
  let score = 0;
  const scope = (candidate.scope ?? "").toLowerCase();

  if (scope.includes("wa_username")) score += 260;
  if (label === "from-t") score += 220;
  if (label === "from-t-no-prefix") score += 180;
  if (label === "through-ampersand") score += 130;
  if (label === "through-ampersand-no-prefix") score += 90;
  if (label === "raw") score -= 120;

  if (ticket.includes("EwD4A+pv") || ticket.includes("EwDoA+pv")) score += 260;
  if (ticket.includes("GwAmAru")) score -= 260;
  if (candidate.path.includes("\\Microsoft\\TokenBroker\\Cache\\")) score += 40;

  if (
    lastSuccess &&
    candidate.path === lastSuccess.sourcePath &&
    label === lastSuccess.variantLabel
  ) {
    score += 5000;
  }

  score += Math.floor(candidate.expiresAtMs / 1000) % 100;
  return score;
}

function collectCandidates(): TokenCandidate[] {
  const candidates: TokenCandidate[] = [];
  const seen = new Set<string>();

  for (const dir of cacheDirs) {
    if (!existsSync(dir)) continue;

    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (!entry.isFile() || !entry.name.endsWith(".tbres")) continue;
      const path = join(dir, entry.name);

      try {
        const text = readFileSync(path, "utf16le").replace(/\0+$/g, "");
        const parsed = JSON.parse(text);
        const system = parsed?.TBDataStoreObject?.ObjectData?.SystemDefinedProperties;
        const responseBytes = system?.ResponseBytes?.Value;
        if (typeof responseBytes !== "string") continue;

        const payload = decodeDpapiBase64(responseBytes);
        if (!payload) continue;

        const raw = extractMarkerValue(payload, "WTRes_Token");
        if (!raw) continue;

        const variants = buildTicketVariants(raw);
        if (variants.length === 0) continue;

        const expiresAtMs = parseTokenbrokerFiletime(system?.Expiration?.Value);
        if (expiresAtMs <= Date.now()) continue;

        const signature = variants.map((v) => v.ticket).join("||");
        if (seen.has(signature)) continue;
        seen.add(signature);

        candidates.push({
          path,
          scope:
            extractMarkerValue(payload, "scope=") ??
            extractMarkerValue(payload, "WA_Scope"),
          expiresAtMs,
          expiresAt: new Date(expiresAtMs).toISOString(),
          variants,
        });
      } catch {
        // skip invalid cache entries
      }
    }
  }

  candidates.sort((a, b) => b.expiresAtMs - a.expiresAtMs);
  return candidates;
}

async function exchangeUserAuth(rpsTicket: string) {
  const response = await fetch("https://user.auth.xboxlive.com/user/authenticate", {
    method: "POST",
    headers: {
      "x-xbl-contract-version": "1",
      "content-type": "application/json",
    },
    body: JSON.stringify({
      RelyingParty: "http://auth.xboxlive.com",
      TokenType: "JWT",
      Properties: {
        AuthMethod: "RPS",
        SiteName: "user.auth.xboxlive.com",
        RpsTicket: rpsTicket,
      },
    }),
  });

  return {
    status: response.status,
    text: await response.text(),
  };
}

async function exchangeXsts(userToken: string, relyingParty: string) {
  const response = await fetch("https://xsts.auth.xboxlive.com/xsts/authorize", {
    method: "POST",
    headers: {
      "x-xbl-contract-version": "1",
      "content-type": "application/json",
    },
    body: JSON.stringify({
      Properties: { SandboxId: "RETAIL", UserTokens: [userToken] },
      RelyingParty: relyingParty,
      TokenType: "JWT",
    }),
  });

  return {
    status: response.status,
    text: await response.text(),
  };
}

async function launcherLoginProbe(xtoken: string) {
  const response = await fetch("https://api.minecraftservices.com/launcher/login", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ platform: "ONESTORE", xtoken }),
  });

  return {
    status: response.status,
    text: await response.text(),
  };
}

async function launcherLoginWithRetry(xtoken: string, retries = 2) {
  for (let attempt = 1; attempt <= retries; attempt += 1) {
    const result = await launcherLoginProbe(xtoken);
    if (result.status !== 429 || attempt >= retries) {
      return { ...result, attempt };
    }
    await sleep(300 * attempt);
  }
  return { status: 0, text: "", attempt: retries };
}

const candidates = collectCandidates();
const lastSuccess = readLastSuccessState();

console.log(
  JSON.stringify({
    stage: "collect",
    candidates: candidates.length,
    scannedDirs: cacheDirs,
    maxCandidates,
    maxAttempts,
    delayMs,
    stateLoaded: Boolean(lastSuccess),
    statePath,
  }),
);

for (const [index, candidate] of candidates.slice(0, 6).entries()) {
  console.log(
    JSON.stringify({
      stage: "candidate",
      rank: index + 1,
      expiresAt: candidate.expiresAt,
      scope: candidate.scope,
      variants: candidate.variants.map((v) => ({ label: v.label, preview: preview(v.ticket) })),
      path: candidate.path,
    }),
  );
}

const attempts: Attempt[] = [];
const seenTickets = new Set<string>();
for (const candidate of candidates.slice(0, maxCandidates)) {
  for (const variant of candidate.variants) {
    if (seenTickets.has(variant.ticket)) {
      continue;
    }
    seenTickets.add(variant.ticket);
    attempts.push({
      candidate,
      label: variant.label,
      ticket: variant.ticket,
      score: rankAttempt(candidate, variant.label, variant.ticket, lastSuccess),
    });
  }
}

attempts.sort((a, b) => b.score - a.score);

const attemptList = fullSearch ? attempts : attempts.slice(0, maxAttempts);
console.log(
  JSON.stringify({
    stage: "plan",
    attempts: attemptList.length,
    fullSearch,
    top: attemptList.slice(0, 6).map((attempt) => ({
      score: attempt.score,
      label: attempt.label,
      scope: attempt.candidate.scope,
      preview: preview(attempt.ticket),
      path: attempt.candidate.path,
    })),
  }),
);

let attemptIndex = 0;
for (const attempt of attemptList) {
  attemptIndex += 1;
  const userAuth = await exchangeUserAuth(attempt.ticket);
  console.log(
    JSON.stringify({
      stage: "user.auth",
      attempt: attemptIndex,
      score: attempt.score,
      label: attempt.label,
      scope: attempt.candidate.scope,
      expiresAt: attempt.candidate.expiresAt,
      path: attempt.candidate.path,
      status: userAuth.status,
      preview: preview(attempt.ticket),
      bodyPreview: userAuth.text.slice(0, bodyPreview),
    }),
  );

  if (userAuth.status !== 200) {
    await sleep(delayMs);
    continue;
  }

  const parsed = JSON.parse(userAuth.text) as {
    Token: string;
    DisplayClaims?: { xui?: Array<{ uhs?: string }> };
  };
  const userToken = parsed.Token;
  const uhs = parsed.DisplayClaims?.xui?.[0]?.uhs;
  if (!userToken || !uhs) {
    await sleep(delayMs);
    continue;
  }

  const rpList = allowAltRelyingParties
    ? [preferredRelyingParty, ...altRelyingParties]
    : [preferredRelyingParty];

  for (const relyingParty of rpList) {
    const xsts = await exchangeXsts(userToken, relyingParty);
    console.log(
      JSON.stringify({
        stage: "xsts",
        attempt: attemptIndex,
        relyingParty,
        status: xsts.status,
        bodyPreview: xsts.text.slice(0, bodyPreview),
      }),
    );
    if (xsts.status !== 200) {
      await sleep(delayMs);
      continue;
    }

    const xstsJson = JSON.parse(xsts.text) as { Token?: string };
    if (!xstsJson.Token) {
      await sleep(delayMs);
      continue;
    }

    const xbl3 = `XBL3.0 x=${uhs};${xstsJson.Token}`;
    const launcher = await launcherLoginWithRetry(xbl3, 2);
    console.log(
      JSON.stringify({
        stage: "launcher.login",
        attempt: attemptIndex,
        retryAttempt: launcher.attempt,
        relyingParty,
        status: launcher.status,
        bodyPreview: launcher.text.slice(0, bodyPreview),
      }),
    );

    if (launcher.status === 200) {
      writeLastSuccessState({
        sourcePath: attempt.candidate.path,
        variantLabel: attempt.label,
        relyingParty,
        ticketPrefix: attempt.ticket.slice(0, 24),
        expiresAt: attempt.candidate.expiresAt,
        savedAt: new Date().toISOString(),
      });
      console.log(
        JSON.stringify({
          stage: "result",
          launcherLogin: "success",
          attempt: attemptIndex,
          relyingParty,
          stateSaved: true,
        }),
      );
      process.exit(0);
    }

    await sleep(delayMs);
  }
}

console.log(
  JSON.stringify({
    stage: "result",
    launcherLogin: "not-found",
    attemptsTried: attemptIndex,
  }),
);

process.exit(2);
