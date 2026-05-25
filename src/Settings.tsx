import { useEffect, useState, useTransition } from "react";
import {
  listProviderProfiles,
  saveProviderProfile,
  saveProviderProfilesBatch,
  deleteProviderProfile,
  startCompatApiServer,
  stopCompatApiServer,
  getCompatApiStatus,
  type CompatApiStatus,
  type ProviderProfileRecord,
  type ProviderProfileUpsertInput,
} from "./desktop";
import { useLanguage, type Language } from "./i18n";
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

function formatRouteSummary(profile: ProviderProfileRecord, t: (key: string, ...args: string[]) => string): string {
  const models = readExactModels(profile.extraJson);
  const prefixes = readModelPrefixes(profile.extraJson);

  if (models.length > 0) {
    return t("settings.models", models.join(", "));
  }

  if (prefixes.length > 0) {
    return t("settings.prefixes", prefixes.join(", "));
  }

  return t("settings.defaultRoute");
}

function formatApiFormat(value: string): string {
  if (value === "anthropic") {
    return "Anthropic";
  }

  if (value === "custom") {
    return "Custom";
  }

  return "OpenAI";
}

function profileToEditorJson(profile: EditableProviderProfile): string {
  const extra = parseExtraJson(profile.extraJson);
  const prefixes = splitList(profile.modelPrefixesText);
  const models = splitList(profile.modelsText);

  delete extra.model_prefixes;
  delete extra.modelPrefixes;
  delete extra.models;

  const obj: Record<string, unknown> = {
    id: profile.id,
    displayName: profile.displayName,
    providerKey: profile.providerKey,
    apiFormat: profile.apiFormat,
  };

  if (profile.baseUrl) obj.baseUrl = profile.baseUrl;
  if (profile.apiKeyEnv) obj.apiKeyEnv = profile.apiKeyEnv;
  obj.enabled = profile.enabled;
  if (prefixes.length > 0) obj.modelPrefixes = prefixes;
  if (models.length > 0) obj.models = models;
  if (Object.keys(extra).length > 0) obj.extraJson = extra;

  return JSON.stringify(obj, null, 2);
}

function editorJsonToProfile(json: string): EditableProviderProfile {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch (e) {
    throw new Error(`Invalid JSON: ${(e as Error).message}`);
  }

  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("Profile must be a JSON object");
  }

  const record = parsed as Record<string, unknown>;
  const displayName = normalizeNullableText(record.displayName ?? record.name) ?? "";
  const providerKey = normalizeNullableText(record.providerKey) ?? "";

  const extraSource = record.extraJson ?? record.extra_json;
  const rawExtraJson =
    typeof extraSource === "object" && extraSource !== null
      ? JSON.stringify(extraSource)
      : normalizeNullableText(extraSource as string | undefined);

  const modelPrefixes = normalizeStringList(
    record.modelPrefixes ?? record.model_prefixes ?? [],
  );
  const models = normalizeStringList(record.models ?? []);

  return {
    id: normalizeNullableText(record.id) ?? crypto.randomUUID(),
    displayName,
    providerKey,
    baseUrl: normalizeNullableText(record.baseUrl ?? record.base_url ?? record.url),
    apiFormat: normalizeApiFormat(record.apiFormat),
    apiKeyEnv: normalizeNullableText(record.apiKeyEnv ?? record.api_key_env),
    enabled: typeof record.enabled === "boolean" ? record.enabled : true,
    extraJson: serializeRoutingExtra(rawExtraJson, modelPrefixes, models),
    modelPrefixesText: modelPrefixes.join(", "),
    modelsText: models.join(", "),
  };
}

function TagInput({
  values,
  onChange,
  placeholder,
  addLabel,
}: {
  values: string;
  onChange: (value: string) => void;
  placeholder: string;
  addLabel: string;
}) {
  const items = splitList(values);
  const [inputValue, setInputValue] = useState("");

  const addItem = () => {
    const trimmed = inputValue.trim();
    if (trimmed && !items.includes(trimmed)) {
      onChange([...items, trimmed].join(", "));
      setInputValue("");
    }
  };

  const removeItem = (index: number) => {
    onChange(items.filter((_, i) => i !== index).join(", "));
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addItem();
    }
  };

  return (
    <div className="tag-input">
      <div className="tag-list">
        {items.map((item, i) => (
          <span key={i} className="tag">
            <span className="tag-text">{item}</span>
            <button
              type="button"
              className="tag-remove"
              onClick={() => removeItem(i)}
            >
              &times;
            </button>
          </span>
        ))}
      </div>
      <div className="tag-input-row">
        <input
          type="text"
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
        />
        <button type="button" className="secondary" onClick={addItem} disabled={!inputValue.trim()}>
          {addLabel}
        </button>
      </div>
    </div>
  );
}

function Settings() {
  const { t, language, setLanguage } = useLanguage();
  const [profiles, setProfiles] = useState<ProviderProfileRecord[]>([]);
  const [compatStatus, setCompatStatus] = useState<CompatApiStatus | null>(null);
  const [listenAddress, setListenAddress] = useState("127.0.0.1:8688");
  const [batchInput, setBatchInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();
  const [editingProfile, setEditingProfile] = useState<EditableProviderProfile | null>(null);
  const [editMode, setEditMode] = useState<"form" | "json">("form");
  const [jsonEditorText, setJsonEditorText] = useState("");
  const enabledProfiles = profiles.filter((profile) => profile.enabled);
  const openAiProfiles = profiles.filter((profile) => profile.apiFormat === "openai" || profile.apiFormat === "custom");
  const anthropicProfiles = profiles.filter((profile) => profile.apiFormat === "anthropic");

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
          refreshError instanceof Error ? refreshError.message : t("error.loadSettings"),
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
        setSuccess(t("settings.compatListening", listenAddress));
      } catch (startError) {
        setError(
          startError instanceof Error ? startError.message : t("error.startCompat"),
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
        setSuccess(t("settings.compatStopped"));
      } catch (stopError) {
        setError(
          stopError instanceof Error ? stopError.message : t("error.stopCompat"),
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
        setEditMode("form");
        refresh();
        setSuccess(t("settings.saved", input.displayName));
      } catch (saveError) {
        setError(
          saveError instanceof Error ? saveError.message : t("error.saveProfile"),
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
          throw new Error(t("error.batchEmpty"));
        }

        const saved = await saveProviderProfilesBatch(inputs);
        setBatchInput("");
        setEditingProfile(null);
        setEditMode("form");
        refresh();
        setSuccess(t("settings.imported", String(saved.length)));
      } catch (importError) {
        setError(
          importError instanceof Error ? importError.message : t("error.importProfiles"),
        );
      }
    });
  };

  const handleJsonSave = () => {
    try {
      const parsed = editorJsonToProfile(jsonEditorText);
      handleSaveProfile({
        id: parsed.id,
        providerKey: parsed.providerKey,
        displayName: parsed.displayName,
        baseUrl: parsed.baseUrl,
        apiFormat: parsed.apiFormat,
        apiKeyEnv: parsed.apiKeyEnv,
        enabled: parsed.enabled,
        extraJson: parsed.extraJson,
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : "Invalid JSON input");
    }
  };

  const handleDeleteProfile = async (id: string, displayName: string) => {
    if (!window.confirm(t("settings.deleteConfirm", displayName))) {
      return;
    }
    startTransition(async () => {
      try {
        setError(null);
        setSuccess(null);
        await deleteProviderProfile(id);
        setEditingProfile(null);
        setEditMode("form");
        refresh();
        setSuccess(t("settings.deleted", displayName));
      } catch (deleteError) {
        setError(
          deleteError instanceof Error ? deleteError.message : t("error.deleteProfile"),
        );
      }
    });
  };

  const handleLanguageChange = (lang: Language) => {
    setLanguage(lang);
  };

  const handleExportJson = () => {
    const data = profiles.map((p) => {
      const prefixes = readModelPrefixes(p.extraJson);
      const models = readExactModels(p.extraJson);
      const extra = parseExtraJson(p.extraJson);
      delete extra.model_prefixes;
      delete extra.modelPrefixes;
      delete extra.models;

      const obj: Record<string, unknown> = {
        displayName: p.displayName,
        providerKey: p.providerKey,
        apiFormat: p.apiFormat,
      };
      if (p.baseUrl) obj.baseUrl = p.baseUrl;
      if (p.apiKeyEnv) obj.apiKeyEnv = p.apiKeyEnv;
      obj.enabled = p.enabled;
      if (prefixes.length > 0) obj.modelPrefixes = prefixes;
      if (models.length > 0) obj.models = models;
      if (Object.keys(extra).length > 0) obj.extraJson = extra;
      return obj;
    });

    const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "provider-profiles.json";
    a.click();
    URL.revokeObjectURL(url);
    setSuccess(t("settings.exported"));
  };

  const handleImportFile = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      setBatchInput(reader.result as string);
    };
    reader.readAsText(file);
    e.target.value = "";
  };

  return (
    <div className="settings-shell">
      {error ? <section className="notice error">{error}</section> : null}
      {success ? <section className="notice success">{success}</section> : null}

      <section className="settings-block">
        <div className="section-header">
          <h2>{t("language.label")}</h2>
        </div>
        <div className="settings-row">
          <div className="settings-group" style={{ maxWidth: 200 }}>
            <select
              value={language}
              onChange={(e) => handleLanguageChange(e.target.value as Language)}
            >
              <option value="en">{t("language.en")}</option>
              <option value="zh">{t("language.zh")}</option>
            </select>
          </div>
        </div>
      </section>

      <section className="settings-block">
        <div className="settings-row">
          <div className="settings-group grow">
            <label htmlFor="listenAddress">{t("settings.compatApi")}</label>
            <input
              id="listenAddress"
              type="text"
              value={listenAddress}
              onChange={(e) => setListenAddress(e.target.value)}
              placeholder={t("settings.placeholder.listenAddress")}
            />
          </div>
          <div className="settings-actions">
            <button
              type="button"
              onClick={handleStartServer}
              disabled={isPending || compatStatus?.running}
            >
              {t("settings.start")}
            </button>
            <button
              type="button"
              className="secondary"
              onClick={handleStopServer}
              disabled={isPending || !compatStatus?.running}
            >
              {t("settings.stop")}
            </button>
            <button type="button" className="secondary" onClick={refresh} disabled={isPending}>
              {t("settings.refresh")}
            </button>
          </div>
        </div>

        <div className="status-strip">
          <span className={`status-pill ${compatStatus?.running ? "running" : "stopped"}`}>
            {compatStatus?.running ? t("settings.running") : t("settings.stopped")}
          </span>
          <span>{enabledProfiles.length}/{profiles.length} {t("settings.enabled")}</span>
          <span>{t("settings.openaiFormat", String(openAiProfiles.length))}</span>
          <span>{t("settings.anthropicFormat", String(anthropicProfiles.length))}</span>
          <span className="mono">{compatStatus?.listenAddress ?? listenAddress}</span>
          {compatStatus?.startedAt ? (
            <span>{new Date(compatStatus.startedAt).toLocaleString()}</span>
          ) : null}
        </div>
        <div className="compat-help">
          <span>Claude Code endpoint: <code>http://{compatStatus?.listenAddress ?? listenAddress}/v1/messages</code></span>
          <span>OpenAI clients: <code>http://{compatStatus?.listenAddress ?? listenAddress}/v1/chat/completions</code></span>
        </div>
      </section>

      <section className="settings-block">
        <div className="section-header">
          <h2>{t("settings.accountPool")}</h2>
          <div className="settings-actions">
            <button type="button" onClick={() => { setEditingProfile(createEmptyProfile()); setEditMode("form"); }}>
              {t("settings.new")}
            </button>
            <label className="import-file-label">
              <input type="file" accept=".json,application/json" onChange={handleImportFile} hidden />
              {t("settings.importFile")}
            </label>
            <button type="button" className="secondary" onClick={handleExportJson} disabled={profiles.length === 0}>
              {t("settings.exportJson")}
            </button>
          </div>
        </div>

        <details className="batch-import">
          <summary className="batch-import-summary">{t("settings.batchImport")}</summary>
          <div className="batch-import-body">
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
    "modelPrefixes": ["deepseek-", "claude-"]
  }
]`}
            />
            <div className="settings-actions">
              <button type="button" onClick={handleBatchImport} disabled={isPending || !batchInput.trim()}>
                {t("settings.importBatch")}
              </button>
            </div>
          </div>
        </details>

        {editingProfile ? (
          <div className="editor-panel">
            <div className="editor-tabs">
              <button
                type="button"
                className={`editor-tab${editMode === "form" ? " active" : ""}`}
                onClick={() => {
                  if (editMode === "json") {
                    try {
                      const parsed = editorJsonToProfile(jsonEditorText);
                      setEditingProfile(parsed);
                      setEditMode("form");
                    } catch (e) {
                      setError(e instanceof Error ? e.message : "Invalid JSON input");
                    }
                  }
                }}
              >
                {t("settings.formMode")}
              </button>
              <button
                type="button"
                className={`editor-tab${editMode === "json" ? " active" : ""}`}
                onClick={() => {
                  if (editMode === "form") {
                    setJsonEditorText(profileToEditorJson(editingProfile));
                    setEditMode("json");
                  }
                }}
              >
                {t("settings.jsonMode")}
              </button>
            </div>
            {editMode === "form" ? (
              <ProfileForm
                key={editingProfile.id}
                profile={editingProfile}
                onSave={handleSaveProfile}
                onDelete={
                  editingProfile.displayName.trim()
                    ? (id) => handleDeleteProfile(id, editingProfile.displayName)
                    : undefined
                }
                onCancel={() => { setEditingProfile(null); setEditMode("form"); }}
                t={t}
              />
            ) : (
              <div className="json-editor">
                <textarea
                  className="json-editor-textarea"
                  value={jsonEditorText}
                  onChange={(e) => setJsonEditorText(e.target.value)}
                  spellCheck={false}
                />
                <div className="settings-actions json-editor-actions">
                  <button type="button" onClick={handleJsonSave}>
                    {t("settings.form.save")}
                  </button>
                  {editingProfile.displayName.trim() ? (
                    <button
                      type="button"
                      className="danger"
                      onClick={() => handleDeleteProfile(editingProfile.id, editingProfile.displayName)}
                    >
                      {t("settings.delete")}
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => { setEditingProfile(null); setEditMode("form"); }}
                  >
                    {t("settings.form.cancel")}
                  </button>
                </div>
              </div>
            )}
          </div>
        ) : null}

        <div className="profiles-table-shell">
          <table className="profiles-table">
            <thead>
              <tr>
                <th>{t("settings.table.name")}</th>
                <th>{t("settings.table.format")}</th>
                <th>{t("settings.table.route")}</th>
                <th>{t("settings.table.baseUrl")}</th>
                <th>{t("settings.table.keyEnv")}</th>
                <th>{t("settings.table.status")}</th>
                <th>{t("settings.table.actions")}</th>
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
                  <td>{formatApiFormat(profile.apiFormat)}</td>
                  <td>{formatRouteSummary(profile, t)}</td>
                  <td className="mono">{profile.baseUrl ?? t("settings.defaultRoute")}</td>
                  <td className="mono">{profile.apiKeyEnv ?? "none"}</td>
                  <td>
                    <span className={`status-pill ${profile.enabled ? "running" : "stopped"}`}>
                      {profile.enabled ? t("settings.enabled2") : t("settings.disabled")}
                    </span>
                  </td>
                  <td className="actions-cell">
                    <div className="row-actions">
                      <button
                        type="button"
                        className="secondary"
                        onClick={() => { setEditingProfile(createEditableProfile(profile)); setEditMode("form"); }}
                      >
                        {t("settings.edit")}
                      </button>
                      <button
                        type="button"
                        className="danger"
                        onClick={() => handleDeleteProfile(profile.id, profile.displayName)}
                      >
                        {t("settings.delete")}
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {profiles.length === 0 ? <p className="empty">{t("settings.noProfiles")}</p> : null}
        </div>
      </section>
    </div>
  );
}

function ProfileForm({
  profile,
  onSave,
  onDelete,
  onCancel,
  t,
}: {
  profile: EditableProviderProfile;
  onSave: (input: ProviderProfileUpsertInput) => void;
  onDelete?: (id: string) => void;
  onCancel: () => void;
  t: (key: string, ...args: string[]) => string;
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
          <label htmlFor="displayName">{t("settings.form.name")}</label>
          <input
            id="displayName"
            type="text"
            value={form.displayName}
            onChange={(e) => setForm({ ...form, displayName: e.target.value })}
            required
          />
        </div>

        <div className="settings-group">
          <label htmlFor="providerKey">{t("settings.form.providerId")}</label>
          <input
            id="providerKey"
            type="text"
            value={form.providerKey}
            onChange={(e) => setForm({ ...form, providerKey: e.target.value })}
            placeholder={t("settings.placeholder.providerId")}
            required
          />
        </div>

        <div className="settings-group">
          <label htmlFor="apiFormat">{t("settings.form.apiFormat")}</label>
          <select
            id="apiFormat"
            value={form.apiFormat}
            onChange={(e) => setForm({ ...form, apiFormat: e.target.value })}
          >
            <option value="openai">{t("settings.form.openai")}</option>
            <option value="anthropic">{t("settings.form.anthropic")}</option>
            <option value="custom">{t("settings.form.custom")}</option>
          </select>
        </div>

        <div className="settings-group">
          <label htmlFor="apiKeyEnv">{t("settings.form.apiKeyEnv")}</label>
          <input
            id="apiKeyEnv"
            type="text"
            value={form.apiKeyEnv ?? ""}
            onChange={(e) => setForm({ ...form, apiKeyEnv: e.target.value || null })}
            placeholder={t("settings.placeholder.keyEnv")}
          />
        </div>

        <div className="settings-group full-width">
          <label htmlFor="baseUrl">{t("settings.form.baseUrl")}</label>
          <input
            id="baseUrl"
            type="text"
            value={form.baseUrl ?? ""}
            onChange={(e) => setForm({ ...form, baseUrl: e.target.value || null })}
            placeholder={t("settings.placeholder.baseUrl")}
          />
        </div>

        <div className="settings-group full-width">
          <label>{t("settings.form.routePrefixes")}</label>
          <TagInput
            values={form.modelPrefixesText}
            onChange={(v) => setForm({ ...form, modelPrefixesText: v })}
            placeholder={t("settings.placeholder.routePrefixes")}
            addLabel={t("settings.form.addPrefix")}
          />
        </div>

        <div className="settings-group full-width">
          <label>{t("settings.form.exactModels")}</label>
          <TagInput
            values={form.modelsText}
            onChange={(v) => setForm({ ...form, modelsText: v })}
            placeholder={t("settings.placeholder.exactModels")}
            addLabel={t("settings.form.addModel")}
          />
        </div>
      </div>

      <label className="checkbox-row">
        <input
          type="checkbox"
          checked={form.enabled}
          onChange={(e) => setForm({ ...form, enabled: e.target.checked })}
        />
        {t("settings.form.enabled")}
      </label>

      <div className="settings-actions profile-form-actions">
        <button type="submit">{t("settings.form.save")}</button>
        {onDelete ? (
          <button type="button" className="danger" onClick={() => onDelete(form.id)}>
            {t("settings.delete")}
          </button>
        ) : null}
        <button type="button" className="secondary" onClick={onCancel}>
          {t("settings.form.cancel")}
        </button>
      </div>
    </form>
  );
}

export default Settings;
