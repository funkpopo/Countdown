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
  phase5Complete: boolean;
  phase6Complete: boolean;
  phase7Complete: boolean;
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
  startedAfter?: string;
  startedBefore?: string;
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

export type ClaudeOverview = {
  dataDir: string;
  dataDirExists: boolean;
  sessionCount: number;
  requestCount: number;
  todayUsage: DailyUsageRecord | null;
  recentRequests: RequestRecordListItem[];
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

export type DateRangeInput = {
  startDate: string;
  endDate: string;
};

export type CombinedUsage = {
  startDate: string;
  endDate: string;
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

export type UsageHistogramInput = {
  period: "today" | "week" | "month";
  granularity: "hour" | "day";
};

export type UsageHistogramBucket = {
  bucket: string;
  label: string;
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
};

export type UsageHistogram = {
  period: "today" | "week" | "month";
  granularity: "hour" | "day";
  buckets: UsageHistogramBucket[];
  lastRefreshAt: string;
};

export type CompatApiStatus = {
  running: boolean;
  listenAddress: string;
  startedAt: string | null;
  profilesCount: number;
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

export async function saveProviderProfilesBatch(
  inputs: ProviderProfileUpsertInput[],
): Promise<ProviderProfileRecord[]> {
  return invoke<ProviderProfileRecord[]>("save_provider_profiles_batch", { inputs });
}

export async function deleteProviderProfile(id: string): Promise<void> {
  return invoke<void>("delete_provider_profile", { id });
}

export async function getCodexOverview(): Promise<CodexOverview> {
  return invoke<CodexOverview>("get_codex_overview");
}

export async function getClaudeCodeOverview(): Promise<ClaudeOverview> {
  return invoke<ClaudeOverview>("get_claude_code_overview");
}

export async function getCombinedTodayUsage(): Promise<CombinedTodayUsage> {
  return invoke<CombinedTodayUsage>("get_combined_today_usage");
}

export async function getCombinedUsage(range: DateRangeInput): Promise<CombinedUsage> {
  return invoke<CombinedUsage>("get_combined_usage", { range });
}

export async function getCombinedUsageTotal(): Promise<CombinedUsage> {
  return invoke<CombinedUsage>("get_combined_usage_total");
}

export async function getUsageHistogram(input: UsageHistogramInput): Promise<UsageHistogram> {
  return invoke<UsageHistogram>("get_usage_histogram", { input });
}

export async function listFilteredRequests(
  filter: RequestFilterInput,
): Promise<PaginatedRequestRecords> {
  return invoke<PaginatedRequestRecords>("list_filtered_requests", { filter });
}

export async function getRequestDetail(id: string): Promise<RequestRecordDetail> {
  return invoke<RequestRecordDetail>("get_request_detail", { id });
}

export async function startCompatApiServer(listenAddress: string): Promise<CompatApiStatus> {
  return invoke<CompatApiStatus>("start_compat_api_server", { listenAddress });
}

export async function stopCompatApiServer(): Promise<CompatApiStatus> {
  return invoke<CompatApiStatus>("stop_compat_api_server");
}

export async function getCompatApiStatus(): Promise<CompatApiStatus> {
  return invoke<CompatApiStatus>("get_compat_api_status");
}
