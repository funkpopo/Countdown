import { useEffect, useState, useTransition } from "react";
import {
  listProviderProfiles,
  saveProviderProfile,
  saveProviderProfilesBatch,
  startCompatApiServer,
  stopCompatApiServer,
  getCompatApiStatus,
  runManagedLaunch,
  type CompatApiStatus,
  type ManagedLaunchInput,
  type ManagedLaunchResult,
  type ProviderProfileRecord,
  type ProviderProfileUpsertInput,
} from "./desktop";
import "./Settings.css";

type EditableProviderProfile = ProviderProfileUpsertInput & {
  modelPrefixesText: string;
  modelsText: string;
};

function normalizeNullableText(value: unknown): string | null {
  if (typeof value !== "string") {
    return null;
  }

  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function splitList(value: string): string[] {
  return value
    .split(/[,\n]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function splitShellArgs(value: string): string[] {
  const args: string[] = [];
  const pattern = /"([^"]*)"|'([^']*)'|[^\s]+/g;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(value)) !== null) {
    args.push(match[1] ?? match[2] ?? match[0]);
  }

  return args;
}

function normalizeStringList(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value
      .filter((item): item is string => typeof item === "string")
      .map((item) => item.trim())
      .filter(Boolean);
  }

  if (typeof value === "string") {
    return splitList(value);
  }

  return [];
}

function parseExtraJson(value: string | null | undefined): Record<string, unknown> {
  if (!value) {
    return {};
  }

  try {
    const parsed = JSON.parse(value) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function readExtraList(extraJson: string | null | undefined, key: string): string[] {
  return normalizeStringList(parseExtraJson(extraJson)[key]);
}

function readModelPrefixes(extraJson: string | null | undefined): string[] {
  const snakeCase = readExtraList(extraJson, "model_prefixes");
  return snakeCase.length > 0 ? snakeCase : readExtraList(extraJson, "modelPrefixes");
}

function readExactModels(extraJson: string | null | undefined): string[] {
  return readExtraList(extraJson, "models");
}

function serializeRoutingExtra(
  baseExtraJson: string | null | undefined,
  modelPrefixes: string[],
  models: string[],
): string | null {
  const extra = parseExtraJson(baseExtraJson);

  delete extra.model_prefixes;
  delete extra.modelPrefixes;
  delete extra.models;

  if (modelPrefixes.length > 0) {
    extra.model_prefixes = modelPrefixes;
  }

  if (models.length > 0) {
    extra.models = models;
  }

  return Object.keys(extra).length > 0 ? JSON.stringify(extra) : null;
}

function createEditableProfile(
  profile: ProviderProfileRecord | ProviderProfileUpsertInput,
): EditableProviderProfile {
  return {
    ...profile,
    modelPrefixesText: readModelPrefixes(profile.extraJson).join(", "),
    modelsText: readExactModels(profile.extraJson).join(", "),
  };
}

function createEmptyProfile(): EditableProviderProfile {
  return createEditableProfile({
    id: crypto.randomUUID(),
    providerKey: "",
    displayName: "",
    baseUrl: null,
    apiFormat: "openai",
    apiKeyEnv: null,
    enabled: true,
    extraJson: null,
  });
}

function normalizeApiFormat(value: unknown): "openai" | "anthropic" | "custom" {
  const format = typeof value === "string" ? value.trim().toLowerCase() : "";

  if (format === "anthropic") {
    return "anthropic";
  }

  if (format === "custom") {
    return "custom";
  }

  return "openai";
}

function getRequiredString(entry: Record<string, unknown>, keys: string[], label: string): string {
  for (const key of keys) {
    const value = normalizeNullableText(entry[key]);
    if (value) {
      return value;
    }
  }

  throw new Error(`Missing ${label}.`);
}

function normalizeBatchEntry(entry: unknown): ProviderProfileUpsertInput {
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
    throw new Error("Each imported profile must be a JSON object.");
  }

  const record = entry as Record<string, unknown>;
  const displayName = getRequiredString(record, ["displayName", "display_name", "name"], "displayName");
  const providerKey = getRequiredString(record, ["providerKey", "provider_key"], "providerKey");
  const extraSource = record.extraJson ?? record.extra_json;
  const rawExtraJson =
    typeof extraSource === "string"
      ? normalizeNullableText(extraSource)
      : extraSource && typeof extraSource === "object"
        ? JSON.stringify(extraSource)
        : null;

  const modelPrefixes = normalizeStringList(
    record.modelPrefixes ?? record.model_prefixes ?? record.prefixes,
  );
  const models = normalizeStringList(record.models);

  return {
    id: normalizeNullableText(record.id) ?? crypto.randomUUID(),
    providerKey,
    displayName,
    baseUrl: normalizeNullableText(record.baseUrl ?? record.base_url ?? record.url),
    apiFormat: normalizeApiFormat(record.apiFormat ?? record.api_format ?? record.protocol ?? record.format),
    apiKeyEnv: normalizeNullableText(record.apiKeyEnv ?? record.api_key_env),
    enabled: typeof record.enabled === "boolean" ? record.enabled : true,
    extraJson: serializeRoutingExtra(rawExtraJson, modelPrefixes, models),
  };
}

function parseBatchProfiles(raw: string): ProviderProfileUpsertInput[] {
  const trimmed = raw.trim();
  if (!trimmed) {
    return [];
  }

  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (Array.isArray(parsed)) {
      return parsed.map(normalizeBatchEntry);
    }

    return [normalizeBatchEntry(parsed)];
  } catch {
    return trimmed
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => normalizeBatchEntry(JSON.parse(line) as unknown));
  }
}

function formatRouteSummary(profile: ProviderProfileRecord): string {
  const models = readExactModels(profile.extraJson);
  const prefixes = readModelPrefixes(profile.extraJson);

  if (models.length > 0) {
    return `models: ${models.join(", ")}`;
  }

  if (prefixes.length > 0) {
    return `prefixes: ${prefixes.join(", ")}`;
  }

  return "default";
}

function Settings() {
  const [profiles, setProfiles] = useState<ProviderProfileRecord[]>([]);
  const [compatStatus, setCompatStatus] = useState<CompatApiStatus | null>(null);
  const [listenAddress, setListenAddress] = useState("127.0.0.1:8688");
  const [batchInput, setBatchInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();
  const [editingProfile, setEditingProfile] = useState<EditableProviderProfile | null>(null);
  const [launchForm, setLaunchForm] = useState<ManagedLaunchInput>({
    provider: "codex",
    executable: "codex",
    args: [],
    stdin: null,
    cwd: null,
    model: null,
  });
  const [launchArgsText, setLaunchArgsText] = useState("");
  const [launchResult, setLaunchResult] = useState<ManagedLaunchResult | null>(null);

  const refresh = () => {
    startTransition(async () => {
      try {
        setError(null);
        const [profilesList, status] = await Promise.all([
          listProviderProfiles(),
          getCompatApiStatus(),
        ]);
        setProfiles(profilesList);
        setCompatStatus(status);
        if (status.listenAddress) {
          setListenAddress(status.listenAddress);
        }
      } catch (refreshError) {
        setError(
          refreshError instanceof Error ? refreshError.message : "Failed to load settings.",
        );
      }
    });
  };

  useEffect(() => {
    refresh();
  }, []);

  const handleStartServer = async () => {
    startTransition(async () => {
      try {
        setError(null);
        setSuccess(null);
        const status = await startCompatApiServer(listenAddress);
        setCompatStatus(status);
        setSuccess(`Compat API listening on ${listenAddress}`);
      } catch (startError) {
        setError(
          startError instanceof Error ? startError.message : "Failed to start Compat API server.",
        );
      }
    });
  };

  const handleStopServer = async () => {
    startTransition(async () => {
      try {
        setError(null);
        setSuccess(null);
        const status = await stopCompatApiServer();
        setCompatStatus(status);
        setSuccess("Compat API stopped");
      } catch (stopError) {
        setError(
          stopError instanceof Error ? stopError.message : "Failed to stop Compat API server.",
        );
      }
    });
  };

  const handleSaveProfile = async (input: ProviderProfileUpsertInput) => {
    startTransition(async () => {
      try {
        setError(null);
        setSuccess(null);
        await saveProviderProfile(input);
        setEditingProfile(null);
        refresh();
        setSuccess(`Saved ${input.displayName}`);
      } catch (saveError) {
        setError(
          saveError instanceof Error ? saveError.message : "Failed to save provider profile.",
        );
      }
    });
  };

  const handleBatchImport = async () => {
    startTransition(async () => {
      try {
        setError(null);
        setSuccess(null);
        const inputs = parseBatchProfiles(batchInput);
        if (inputs.length === 0) {
          throw new Error("Paste a JSON array or JSONL payload first.");
        }

        const saved = await saveProviderProfilesBatch(inputs);
        setBatchInput("");
        setEditingProfile(null);
        refresh();
        setSuccess(`Imported ${saved.length} provider profiles`);
      } catch (importError) {
        setError(
          importError instanceof Error ? importError.message : "Failed to import provider profiles.",
        );
      }
    });
  };

  const handleManagedLaunch = async () => {
    startTransition(async () => {
      try {
        setError(null);
        setSuccess(null);
        setLaunchResult(null);
        const result = await runManagedLaunch({
          ...launchForm,
          executable: launchForm.executable.trim(),
          args: splitShellArgs(launchArgsText),
          stdin: normalizeNullableText(launchForm.stdin),
          cwd: normalizeNullableText(launchForm.cwd),
          model: normalizeNullableText(launchForm.model),
        });
        setLaunchResult(result);
        setSuccess(`Captured ${result.provider} managed launch`);
      } catch (launchError) {
        setError(
          launchError instanceof Error ? launchError.message : "Failed to run managed launch.",
        );
      }
    });
  };

  return (
    <div className="settings-shell">
      {error ? <section className="notice error">{error}</section> : null}
      {success ? <section className="notice success">{success}</section> : null}

      <section className="settings-block">
        <div className="section-header">
          <h2>Managed Launch</h2>
        </div>

        <div className="profile-grid">
          <div className="settings-group">
            <label htmlFor="launchProvider">Provider</label>
            <select
              id="launchProvider"
              value={launchForm.provider}
              onChange={(event) => {
                const provider = event.target.value as "codex" | "claude_code";
                setLaunchForm({
                  ...launchForm,
                  provider,
                  executable: provider === "codex" ? "codex" : "claude",
                });
              }}
            >
              <option value="codex">Codex</option>
              <option value="claude_code">Claude Code</option>
            </select>
          </div>

          <div className="settings-group">
            <label htmlFor="launchExecutable">Executable</label>
            <input
              id="launchExecutable"
              type="text"
              value={launchForm.executable}
              onChange={(event) => setLaunchForm({ ...launchForm, executable: event.target.value })}
              placeholder="codex"
            />
          </div>

          <div className="settings-group full-width">
            <label htmlFor="launchArgs">Args</label>
            <input
              id="launchArgs"
              type="text"
              value={launchArgsText}
              onChange={(event) => setLaunchArgsText(event.target.value)}
              placeholder='--output-format stream-json -p "Summarize this project"'
            />
          </div>

          <div className="settings-group">
            <label htmlFor="launchCwd">Working Dir</label>
            <input
              id="launchCwd"
              type="text"
              value={launchForm.cwd ?? ""}
              onChange={(event) => setLaunchForm({ ...launchForm, cwd: event.target.value })}
              placeholder="d:\\Projects\\Countdown"
            />
          </div>

          <div className="settings-group">
            <label htmlFor="launchModel">Model</label>
            <input
              id="launchModel"
              type="text"
              value={launchForm.model ?? ""}
              onChange={(event) => setLaunchForm({ ...launchForm, model: event.target.value })}
              placeholder="optional fallback"
            />
          </div>

          <div className="settings-group full-width">
            <label htmlFor="launchStdin">Stdin</label>
            <textarea
              id="launchStdin"
              value={launchForm.stdin ?? ""}
              onChange={(event) => setLaunchForm({ ...launchForm, stdin: event.target.value })}
              placeholder="Optional prompt or JSONL input for a wrapper script"
            />
          </div>
        </div>

        <div className="settings-actions">
          <button type="button" onClick={handleManagedLaunch} disabled={isPending}>
            Run & Capture
          </button>
        </div>

        {launchResult ? (
          <div className="status-strip">
            <span className={`status-pill ${launchResult.status === "completed" ? "running" : "stopped"}`}>
              {launchResult.status}
            </span>
            <span>{launchResult.inputTokens + launchResult.outputTokens} tokens</span>
            <span>{launchResult.durationMs} ms</span>
            <span className="mono">{launchResult.model ?? "model unknown"}</span>
          </div>
        ) : null}
      </section>

      <section className="settings-block">
        <div className="settings-row">
          <div className="settings-group grow">
            <label htmlFor="listenAddress">Listen</label>
            <input
              id="listenAddress"
              type="text"
              value={listenAddress}
              onChange={(e) => setListenAddress(e.target.value)}
              placeholder="127.0.0.1:8688"
            />
          </div>
          <div className="settings-actions">
            <button
              type="button"
              onClick={handleStartServer}
              disabled={isPending || compatStatus?.running}
            >
              Start
            </button>
            <button
              type="button"
              className="secondary"
              onClick={handleStopServer}
              disabled={isPending || !compatStatus?.running}
            >
              Stop
            </button>
            <button type="button" className="secondary" onClick={refresh} disabled={isPending}>
              Refresh
            </button>
          </div>
        </div>

        <div className="status-strip">
          <span className={`status-pill ${compatStatus?.running ? "running" : "stopped"}`}>
            {compatStatus?.running ? "Running" : "Stopped"}
          </span>
          <span>{compatStatus?.profilesCount ?? profiles.length} profiles</span>
          <span className="mono">{compatStatus?.listenAddress ?? listenAddress}</span>
          {compatStatus?.startedAt ? (
            <span>{new Date(compatStatus.startedAt).toLocaleString()}</span>
          ) : null}
        </div>
      </section>

      <section className="settings-block">
        <div className="section-header">
          <h2>Provider Profiles</h2>
          <div className="settings-actions">
            <button type="button" onClick={() => setEditingProfile(createEmptyProfile())}>
              New
            </button>
          </div>
        </div>

        <div className="batch-import">
          <textarea
            value={batchInput}
            onChange={(e) => setBatchInput(e.target.value)}
            placeholder={`[
  {
    "displayName": "DeepSeek",
    "providerKey": "deepseek",
    "apiFormat": "openai",
    "baseUrl": "https://api.deepseek.com",
    "apiKeyEnv": "DEEPSEEK_API_KEY",
    "modelPrefixes": ["deepseek-"]
  }
]`}
          />
          <div className="settings-actions">
            <button type="button" onClick={handleBatchImport} disabled={isPending}>
              Import Batch
            </button>
          </div>
        </div>

        {editingProfile ? (
          <ProfileForm
            profile={editingProfile}
            onSave={handleSaveProfile}
            onCancel={() => setEditingProfile(null)}
          />
        ) : null}

        <div className="profiles-table-shell">
          <table className="profiles-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Protocol</th>
                <th>Route</th>
                <th>Base URL</th>
                <th>Key Env</th>
                <th>Status</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {profiles.map((profile) => (
                <tr key={profile.id}>
                  <td>
                    <div className="profile-name-cell">
                      <strong>{profile.displayName}</strong>
                      <span className="mono">{profile.providerKey}</span>
                    </div>
                  </td>
                  <td>{profile.apiFormat}</td>
                  <td>{formatRouteSummary(profile)}</td>
                  <td className="mono">{profile.baseUrl ?? "default"}</td>
                  <td className="mono">{profile.apiKeyEnv ?? "none"}</td>
                  <td>
                    <span className={`status-pill ${profile.enabled ? "running" : "stopped"}`}>
                      {profile.enabled ? "Enabled" : "Disabled"}
                    </span>
                  </td>
                  <td className="actions-cell">
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => setEditingProfile(createEditableProfile(profile))}
                    >
                      Edit
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {profiles.length === 0 ? <p className="empty">No provider profiles.</p> : null}
        </div>
      </section>
    </div>
  );
}

function ProfileForm({
  profile,
  onSave,
  onCancel,
}: {
  profile: EditableProviderProfile;
  onSave: (input: ProviderProfileUpsertInput) => void;
  onCancel: () => void;
}) {
  const [form, setForm] = useState(profile);

  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    onSave({
      id: form.id,
      providerKey: form.providerKey.trim(),
      displayName: form.displayName.trim(),
      baseUrl: normalizeNullableText(form.baseUrl),
      apiFormat: form.apiFormat,
      apiKeyEnv: normalizeNullableText(form.apiKeyEnv),
      enabled: form.enabled,
      extraJson: serializeRoutingExtra(
        form.extraJson,
        splitList(form.modelPrefixesText),
        splitList(form.modelsText),
      ),
    });
  };

  return (
    <form className="profile-form" onSubmit={handleSubmit}>
      <div className="profile-grid">
        <div className="settings-group">
          <label htmlFor="displayName">Name</label>
          <input
            id="displayName"
            type="text"
            value={form.displayName}
            onChange={(e) => setForm({ ...form, displayName: e.target.value })}
            required
          />
        </div>

        <div className="settings-group">
          <label htmlFor="providerKey">Key</label>
          <input
            id="providerKey"
            type="text"
            value={form.providerKey}
            onChange={(e) => setForm({ ...form, providerKey: e.target.value })}
            required
          />
        </div>

        <div className="settings-group">
          <label htmlFor="apiFormat">Protocol</label>
          <select
            id="apiFormat"
            value={form.apiFormat}
            onChange={(e) => setForm({ ...form, apiFormat: e.target.value })}
          >
            <option value="openai">OpenAI</option>
            <option value="anthropic">Anthropic</option>
            <option value="custom">Custom</option>
          </select>
        </div>

        <div className="settings-group">
          <label htmlFor="apiKeyEnv">Key Env</label>
          <input
            id="apiKeyEnv"
            type="text"
            value={form.apiKeyEnv ?? ""}
            onChange={(e) => setForm({ ...form, apiKeyEnv: e.target.value || null })}
            placeholder="OPENAI_API_KEY"
          />
        </div>

        <div className="settings-group full-width">
          <label htmlFor="baseUrl">Base URL</label>
          <input
            id="baseUrl"
            type="text"
            value={form.baseUrl ?? ""}
            onChange={(e) => setForm({ ...form, baseUrl: e.target.value || null })}
            placeholder="https://api.openai.com"
          />
        </div>

        <div className="settings-group full-width">
          <label htmlFor="modelPrefixes">Model Prefixes</label>
          <input
            id="modelPrefixes"
            type="text"
            value={form.modelPrefixesText}
            onChange={(e) => setForm({ ...form, modelPrefixesText: e.target.value })}
            placeholder="gpt-, deepseek-, qwen-"
          />
        </div>

        <div className="settings-group full-width">
          <label htmlFor="models">Exact Models</label>
          <input
            id="models"
            type="text"
            value={form.modelsText}
            onChange={(e) => setForm({ ...form, modelsText: e.target.value })}
            placeholder="gpt-4.1, claude-3-7-sonnet"
          />
        </div>
      </div>

      <label className="checkbox-row">
        <input
          type="checkbox"
          checked={form.enabled}
          onChange={(e) => setForm({ ...form, enabled: e.target.checked })}
        />
        Enabled
      </label>

      <div className="settings-actions">
        <button type="submit">Save</button>
        <button type="button" className="secondary" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}

export default Settings;
