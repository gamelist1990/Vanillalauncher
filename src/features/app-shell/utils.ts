const COMPACT_SIDEBAR_MEDIA = "(max-width: 1100px)";
const LOW_END_POLL_MS = 5000;
const NORMAL_POLL_MS = 2500;

function normalizeDiscoverQuery(value: string) {
  const trimmed = value.trim();
  return trimmed === "0" ? "" : trimmed;
}

function createNoticeId() {
  return `notice-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function createOperationId(prefix: string) {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
}

function shouldUsePerformanceLiteMode() {
  if (typeof navigator === "undefined") {
    return false;
  }

  const connection = (navigator as Navigator & {
    connection?: { effectiveType?: string; saveData?: boolean };
  }).connection;
  const effectiveType = connection?.effectiveType ?? "";
  const saveDataEnabled = connection?.saveData === true;
  const lowBandwidth = effectiveType === "slow-2g" || effectiveType === "2g";

  const cpuThreads = navigator.hardwareConcurrency ?? 8;
  const memory = Number((navigator as Navigator & { deviceMemory?: number }).deviceMemory ?? 8);
  const lowSpec = cpuThreads <= 4 || memory <= 4;

  return saveDataEnabled || lowBandwidth || lowSpec;
}

export {
  COMPACT_SIDEBAR_MEDIA,
  LOW_END_POLL_MS,
  NORMAL_POLL_MS,
  normalizeDiscoverQuery,
  createNoticeId,
  createOperationId,
  shouldUsePerformanceLiteMode,
};
