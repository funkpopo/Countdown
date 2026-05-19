import { invoke } from "@tauri-apps/api/core";

export type BootstrapInfo = {
  productName: string;
  version: string;
  identifier: string;
  appDataDir: string;
  databasePath: string;
  phase0Complete: boolean;
  phase1Complete: boolean;
  phase2Complete: boolean;
  phase3Complete: boolean;
  phase4Complete: boolean;
};

export type DatabaseHealth = {
  databasePath: string;
  exists: boolean;
  writable: boolean;
  schemaVersion: string | null;
  initializedAt: string | null;
  migrationCount: number;
};

export type AppliedMigration = {
  version: number;
  name: string;
  appliedAt: string;
};

export type TableStat = {
  tableName: string;
  rowCount: number;
};

export type ProviderProfileRecord = {
  id: string;
  providerKey: string;
  displayName: string;
  baseUrl: string | null;
  apiFormat: string;
  apiKeyEnv: string | null;
  enabled: boolean;
  extraJson: string | null;
  createdAt: string;
  updatedAt: string;
};

export type ProviderProfileUpsertInput = {
  id: string;
  providerKey: string;
  displayName: string;
  baseUrl: string | null;
  apiFormat: string;
  apiKeyEnv: string | null;
  enabled: boolean;
  extraJson: string | null;
};

export type DatabaseSummary = {
  schemaVersion: string | null;
  initializedAt: string | null;
  appliedMigrations: AppliedMigration[];
  tables: TableStat[];
  providerProfiles: ProviderProfileRecord[];
};

export type DailyUsageRecord = {
  date: string;
  provider: string;
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  requestCount: number;
  streamCount: number;
  nonStreamCount: number;
  avgTtftMs: number | null;
  avgDurationMs: number | null;
  updatedAt: string;
};

export type RequestRecordListItem = {
  id: string;
  provider: string;
  sourceMode: string;
  sessionId: string | null;
  requestId: string | null;
  model: string | null;
  isStream: boolean;
  inputTokens: number;
  outputTokens: number;
  cachedInputTokens: number;
  reasoningTokens: number;
  ttftMs: number | null;
  durationMs: number | null;
  status: string;
  startedAt: string;
  finishedAt: string | null;
  cwd: string | null;
  entrypoint: string | null;
};

export type RequestRecordDetail = {
  id: string;
  provider: string;
  sourceMode: string;
  sessionId: string | null;
  requestId: string | null;
  model: string | null;
  isStream: boolean;
  inputTokens: number;
  outputTokens: number;
  cachedInputTokens: number;
  reasoningTokens: number;
  ttftMs: number | null;
  durationMs: number | null;
  status: string;
  startedAt: string;
  finishedAt: string | null;
  cwd: string | null;
  entrypoint: string | null;
  requestSummaryJson: string | null;
  responseSummaryJson: string | null;
  errorText: string | null;
};

export type RequestFilterInput = {
  provider?: string;
  model?: string;
  isStream?: boolean;
  limit?: number;
  offset?: number;
};

export type PaginatedRequestRecords = {
  records: RequestRecordListItem[];
  total: number;
  limit: number;
  offset: number;
};

export type CodexOverview = {
  dataDir: string;
  dataDirExists: boolean;
  sessionCount: number;
  requestCount: number;
  todayUsage: DailyUsageRecord | null;
  recentRequests: RequestRecordListItem[];
};

export type CodexSyncSummary = {
  dataDir: string;
  dataDirExists: boolean;
  scannedFiles: number;
  importedSessions: number;
  importedRequests: number;
  skippedIncompleteTurns: number;
  sessionCount: number;
  requestCount: number;
  todayUsage: DailyUsageRecord | null;
};

export type ClaudeOverview = {
  dataDir: string;
  dataDirExists: boolean;
  sessionCount: number;
  requestCount: number;
  todayUsage: DailyUsageRecord | null;
  recentRequests: RequestRecordListItem[];
};

export type ClaudeCodeSyncSummary = {
  dataDir: string;
  dataDirExists: boolean;
  scannedFiles: number;
  importedSessions: number;
  importedRequests: number;
  skippedIncompleteSessions: number;
  sessionCount: number;
  requestCount: number;
  todayUsage: DailyUsageRecord | null;
};

export type CombinedTodayUsage = {
  date: string;
  claudeInputTokens: number;
  claudeOutputTokens: number;
  claudeTotalTokens: number;
  claudeRequestCount: number;
  codexInputTokens: number;
  codexOutputTokens: number;
  codexTotalTokens: number;
  codexRequestCount: number;
  combinedInputTokens: number;
  combinedOutputTokens: number;
  combinedTotalTokens: number;
  combinedRequestCount: number;
  lastRefreshAt: string;
};

export async function getBootstrapInfo(): Promise<BootstrapInfo> {
  return invoke<BootstrapInfo>("get_bootstrap_info");
}

export async function initializeLocalDatabase(): Promise<DatabaseHealth> {
  return invoke<DatabaseHealth>("initialize_local_database");
}

export async function databaseHealthcheck(): Promise<DatabaseHealth> {
  return invoke<DatabaseHealth>("database_healthcheck");
}

export async function getDatabaseSummary(): Promise<DatabaseSummary> {
  return invoke<DatabaseSummary>("get_database_summary");
}

export async function listProviderProfiles(): Promise<ProviderProfileRecord[]> {
  return invoke<ProviderProfileRecord[]>("list_provider_profiles");
}

export async function saveProviderProfile(
  input: ProviderProfileUpsertInput,
): Promise<ProviderProfileRecord> {
  return invoke<ProviderProfileRecord>("save_provider_profile", { input });
}

export async function syncCodexSessions(): Promise<CodexSyncSummary> {
  return invoke<CodexSyncSummary>("sync_codex_sessions");
}

export async function getCodexOverview(): Promise<CodexOverview> {
  return invoke<CodexOverview>("get_codex_overview");
}

export async function syncClaudeCodeSessions(): Promise<ClaudeCodeSyncSummary> {
  return invoke<ClaudeCodeSyncSummary>("sync_claude_code_sessions");
}

export async function getClaudeCodeOverview(): Promise<ClaudeOverview> {
  return invoke<ClaudeOverview>("get_claude_code_overview");
}

export async function getCombinedTodayUsage(): Promise<CombinedTodayUsage> {
  return invoke<CombinedTodayUsage>("get_combined_today_usage");
}

export async function listFilteredRequests(
  filter: RequestFilterInput,
): Promise<PaginatedRequestRecords> {
  return invoke<PaginatedRequestRecords>("list_filtered_requests", { filter });
}

export async function getRequestDetail(id: string): Promise<RequestRecordDetail> {
  return invoke<RequestRecordDetail>("get_request_detail", { id });
}
