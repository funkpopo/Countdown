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
