import { useEffect, useState, useTransition } from "react";
import {
  getRequestFilterOptions,
  listFilteredRequests,
  getRequestDetail,
  type RequestFilterInput,
  type RequestFilterOptions,
  type RequestRecordListItem,
  type RequestRecordDetail,
  type PaginatedRequestRecords,
} from "./desktop";
import { useLanguage } from "./i18n";
import "./Requests.css";

function formatNumber(value: number | null | undefined) {
  if (value == null) {
    return "0";
  }

  return new Intl.NumberFormat("en-US").format(value);
}

function formatMs(value: number | null | undefined) {
  if (value == null) {
    return "N/A";
  }

  if (value >= 1000) {
    return `${(value / 1000).toFixed(2)} s`;
  }

  return `${value} ms`;
}

function formatDateTime(value: string | null | undefined) {
  if (!value) {
    return "N/A";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

function formatJsonSummary(value: string) {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

const FILTER_STORAGE_KEY = "countdown.requestFilters.v1";

type RequestSortBy = NonNullable<RequestFilterInput["sortBy"]>;
type RequestSortDir = NonNullable<RequestFilterInput["sortDir"]>;

type RequestFilterState = RequestFilterInput & {
  providers: string[];
  search: string;
  status: string;
  sortBy: RequestSortBy;
  sortDir: RequestSortDir;
  startedAfter: string;
  startedBefore: string;
  model: string;
  modelQuery: string;
};

const DEFAULT_FILTER_STATE: RequestFilterState = {
  providers: [],
  search: "",
  status: "",
  sortBy: "startedAt",
  sortDir: "desc",
  startedAfter: "",
  startedBefore: "",
  model: "",
  modelQuery: "",
  isStream: undefined,
  limit: 50,
  offset: 0,
};

function loadSavedFilters(): RequestFilterState {
  if (typeof window === "undefined") {
    return DEFAULT_FILTER_STATE;
  }

  try {
    const raw = window.localStorage.getItem(FILTER_STORAGE_KEY);
    if (!raw) {
      return DEFAULT_FILTER_STATE;
    }

    const parsed = JSON.parse(raw) as Partial<RequestFilterState>;
    return {
      ...DEFAULT_FILTER_STATE,
      ...parsed,
      providers: Array.isArray(parsed.providers) ? parsed.providers.filter((value): value is string => typeof value === "string") : [],
      search: typeof parsed.search === "string" ? parsed.search : "",
      status: typeof parsed.status === "string" ? parsed.status : "",
      sortBy: parsed.sortBy === "tokens" || parsed.sortBy === "duration" || parsed.sortBy === "model" ? parsed.sortBy : "startedAt",
      sortDir: parsed.sortDir === "asc" ? "asc" : "desc",
      startedAfter: typeof parsed.startedAfter === "string" ? parsed.startedAfter : "",
      startedBefore: typeof parsed.startedBefore === "string" ? parsed.startedBefore : "",
      model: typeof parsed.model === "string" ? parsed.model : "",
      modelQuery: typeof parsed.modelQuery === "string" ? parsed.modelQuery : "",
      isStream: typeof parsed.isStream === "boolean" ? parsed.isStream : undefined,
      limit: typeof parsed.limit === "number" ? parsed.limit : 50,
      offset: typeof parsed.offset === "number" ? parsed.offset : 0,
      provider: typeof parsed.provider === "string" && parsed.provider ? parsed.provider : undefined,
    };
  } catch {
    return DEFAULT_FILTER_STATE;
  }
}

function saveFilters(filter: RequestFilterState) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(FILTER_STORAGE_KEY, JSON.stringify(filter));
}

function RequestDetailDrawer({
  detail,
  onClose,
  t,
}: {
  detail: RequestRecordDetail | null;
  onClose: () => void;
  t: (key: string, ...args: string[]) => string;
}) {
  if (!detail) {
    return null;
  }

  return (
    <div className="drawer-overlay" onClick={onClose}>
      <div className="drawer" onClick={(e) => e.stopPropagation()}>
        <div className="drawer-header">
          <h2>{t("detail.title")}</h2>
          <button type="button" className="close-btn" onClick={onClose}>
            ×
          </button>
        </div>

        <div className="drawer-content">
          <section className="detail-section">
            <h3>{t("detail.basicInfo")}</h3>
            <dl className="detail-grid">
              <div>
                <dt>{t("detail.provider")}</dt>
                <dd>
                  <span className={`provider-badge ${detail.provider}`}>{detail.provider}</span>
                </dd>
              </div>
              <div>
                <dt>{t("detail.sourceMode")}</dt>
                <dd>{detail.sourceMode}</dd>
              </div>
              <div>
                <dt>{t("detail.model")}</dt>
                <dd className="mono">{detail.model ?? t("n/a")}</dd>
              </div>
              <div>
                <dt>{t("detail.streamMode")}</dt>
                <dd>{detail.isStream ? t("yes") : t("no")}</dd>
              </div>
              <div>
                <dt>{t("detail.status")}</dt>
                <dd>{detail.status}</dd>
              </div>
              <div>
                <dt>{t("detail.requestId")}</dt>
                <dd className="mono">{detail.requestId ?? t("n/a")}</dd>
              </div>
              <div>
                <dt>{t("detail.sessionId")}</dt>
                <dd className="mono">{detail.sessionId ?? t("n/a")}</dd>
              </div>
              <div>
                <dt>{t("detail.workingDir")}</dt>
                <dd className="mono">{detail.cwd ?? t("n/a")}</dd>
              </div>
              <div>
                <dt>{t("detail.entrypoint")}</dt>
                <dd className="mono">{detail.entrypoint ?? t("n/a")}</dd>
              </div>
            </dl>
          </section>

          <section className="detail-section">
            <h3>{t("detail.tokenUsage")}</h3>
            <div className="stats-grid">
              <div className="stat-card">
                <span>{t("detail.inputTokens")}</span>
                <strong>{formatNumber(detail.inputTokens)}</strong>
              </div>
              <div className="stat-card">
                <span>{t("detail.outputTokens")}</span>
                <strong>{formatNumber(detail.outputTokens)}</strong>
              </div>
              <div className="stat-card">
                <span>{t("detail.cachedInput")}</span>
                <strong>{formatNumber(detail.cachedInputTokens)}</strong>
              </div>
              <div className="stat-card">
                <span>{t("detail.reasoning")}</span>
                <strong>{formatNumber(detail.reasoningTokens)}</strong>
              </div>
              <div className="stat-card">
                <span>{t("detail.totalTokens")}</span>
                <strong>{formatNumber(detail.inputTokens + detail.outputTokens)}</strong>
              </div>
            </div>
          </section>

          <section className="detail-section">
            <h3>{t("detail.latency")}</h3>
            <div className="stats-grid">
              <div className="stat-card">
                <span>{t("detail.ttft")}</span>
                <strong>{formatMs(detail.ttftMs)}</strong>
              </div>
              <div className="stat-card">
                <span>{t("detail.duration")}</span>
                <strong>{formatMs(detail.durationMs)}</strong>
              </div>
            </div>
          </section>

          <section className="detail-section">
            <h3>{t("detail.timing")}</h3>
            <dl className="detail-grid">
              <div>
                <dt>{t("detail.startedAt")}</dt>
                <dd>{formatDateTime(detail.startedAt)}</dd>
              </div>
              <div>
                <dt>{t("detail.finishedAt")}</dt>
                <dd>{formatDateTime(detail.finishedAt)}</dd>
              </div>
            </dl>
          </section>

          {detail.requestSummaryJson ? (
            <section className="detail-section">
              <h3>{t("detail.requestSummary")}</h3>
              <pre className="json-block">{formatJsonSummary(detail.requestSummaryJson)}</pre>
            </section>
          ) : null}

          {detail.responseSummaryJson ? (
            <section className="detail-section">
              <h3>{t("detail.responseSummary")}</h3>
              <pre className="json-block">{formatJsonSummary(detail.responseSummaryJson)}</pre>
            </section>
          ) : null}

          {detail.errorText ? (
            <section className="detail-section error-section">
              <h3>{t("detail.error")}</h3>
              <pre className="error-block">{detail.errorText}</pre>
            </section>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function renderRequestRow(
  request: RequestRecordListItem,
  onSelect: (id: string) => void,
  t: (key: string, ...args: string[]) => string,
) {
  return (
    <tr key={request.id} onClick={() => onSelect(request.id)} className="request-row">
      <td>
        <div className="primary-cell">
          <strong>{request.model ?? t("model.unknown")}</strong>
          <span>{request.requestId ?? request.id}</span>
        </div>
      </td>
      <td>
        <span className={`provider-badge ${request.provider}`}>{request.provider}</span>
      </td>
      <td>{request.isStream ? t("mode.stream") : t("mode.nonStream")}</td>
      <td>{formatNumber(request.inputTokens)}</td>
      <td>{formatNumber(request.outputTokens)}</td>
      <td>{formatNumber(request.cachedInputTokens)}</td>
      <td>{formatNumber(request.reasoningTokens)}</td>
      <td>{formatMs(request.ttftMs)}</td>
      <td>{formatMs(request.durationMs)}</td>
      <td>{request.status}</td>
      <td>{formatDateTime(request.startedAt)}</td>
    </tr>
  );
}

function Requests() {
  const { t } = useLanguage();
  const [filter, setFilter] = useState<RequestFilterState>(() => loadSavedFilters());
  const [filterOptions, setFilterOptions] = useState<RequestFilterOptions | null>(null);
  const [data, setData] = useState<PaginatedRequestRecords | null>(null);
  const [selectedDetail, setSelectedDetail] = useState<RequestRecordDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);

  const refresh = () => {
    startTransition(async () => {
      try {
        setError(null);
        const result = await listFilteredRequests(filter);
        setData(result);
      } catch (refreshError) {
        setError(
          refreshError instanceof Error ? refreshError.message : t("error.loadRequests"),
        );
      }
    });
  };

  useEffect(() => {
    void getRequestFilterOptions()
      .then(setFilterOptions)
      .catch(() => {
        setFilterOptions({ providers: [], models: [], statuses: [] });
      });
  }, []);

  useEffect(() => {
    saveFilters(filter);
  }, [filter]);

  useEffect(() => {
    refresh();
  }, [filter]);

  const handleSelectRequest = async (id: string) => {
    try {
      setIsLoadingDetail(true);
      const detail = await getRequestDetail(id);
      setSelectedDetail(detail);
    } catch (detailError) {
      setError(
        detailError instanceof Error ? detailError.message : t("error.loadDetail"),
      );
    } finally {
      setIsLoadingDetail(false);
    }
  };

  const handleCloseDetail = () => {
    setSelectedDetail(null);
  };

  const handleProviderChange = (provider: string | undefined) => {
    setFilter((prev) => ({ ...prev, provider, offset: 0 }));
  };

  const handleProvidersChange = (providers: string[]) => {
    setFilter((prev) => ({ ...prev, providers, offset: 0 }));
  };

  const handleModelChange = (model: string | undefined) => {
    setFilter((prev) => ({ ...prev, model: model ?? "", offset: 0 }));
  };

  const handleModelQueryChange = (modelQuery: string) => {
    setFilter((prev) => ({ ...prev, modelQuery, offset: 0 }));
  };

  const handleSearchChange = (search: string) => {
    setFilter((prev) => ({ ...prev, search, offset: 0 }));
  };

  const handleStreamChange = (isStream: boolean | undefined) => {
    setFilter((prev) => ({ ...prev, isStream, offset: 0 }));
  };

  const handleStatusChange = (status: string) => {
    setFilter((prev) => ({ ...prev, status, offset: 0 }));
  };

  const handleDateChange = (key: "startedAfter" | "startedBefore", value: string) => {
    setFilter((prev) => ({ ...prev, [key]: value, offset: 0 }));
  };

  const handleSortChange = (sortBy: RequestSortBy) => {
    setFilter((prev) => ({
      ...prev,
      sortBy,
      offset: 0,
    }));
  };

  const handleSortDirChange = (sortDir: RequestSortDir) => {
    setFilter((prev) => ({
      ...prev,
      sortDir,
      offset: 0,
    }));
  };

  const handleResetFilters = () => {
    setFilter((prev) => ({
      ...DEFAULT_FILTER_STATE,
      limit: prev.limit,
    }));
  };

  const handleNextPage = () => {
    if (data && data.offset + data.limit < data.total) {
      setFilter((prev) => ({ ...prev, offset: (prev.offset ?? 0) + (prev.limit ?? 50) }));
    }
  };

  const handlePrevPage = () => {
    if (data && data.offset > 0) {
      setFilter((prev) => ({ ...prev, offset: Math.max(0, (prev.offset ?? 0) - (prev.limit ?? 50)) }));
    }
  };

  return (
    <div className="requests-page">
      <div className="requests-header">
        <h1>{t("requests.title")}</h1>
        <div className="header-actions">
          <button type="button" className="secondary" onClick={handleResetFilters} disabled={isPending}>
            {t("requests.reset")}
          </button>
          <button type="button" onClick={refresh} disabled={isPending}>
            {t("requests.refresh")}
          </button>
        </div>
      </div>

      {error ? <section className="notice error">{error}</section> : null}

      <section className="filters-panel">
        <div className="filter-group wide">
          <label htmlFor="request-search">{t("filter.search")}</label>
          <input
            id="request-search"
            type="search"
            placeholder={t("filter.searchPlaceholder")}
            value={filter.search}
            onChange={(e) => handleSearchChange(e.target.value)}
          />
        </div>

        <div className="filter-group">
          <label htmlFor="started-after">{t("filter.startedAfter")}</label>
          <input
            id="started-after"
            type="date"
            value={filter.startedAfter}
            onChange={(e) => handleDateChange("startedAfter", e.target.value)}
          />
        </div>

        <div className="filter-group">
          <label htmlFor="started-before">{t("filter.startedBefore")}</label>
          <input
            id="started-before"
            type="date"
            value={filter.startedBefore}
            onChange={(e) => handleDateChange("startedBefore", e.target.value)}
          />
        </div>

        <div className="filter-group">
          <label htmlFor="provider-filter">{t("filter.provider")}</label>
          <select
            id="provider-filter"
            value={filter.provider ?? ""}
            onChange={(e) => handleProviderChange(e.target.value || undefined)}
          >
            <option value="">{t("filter.allProviders")}</option>
            {(filterOptions?.providers ?? []).map((provider) => (
              <option key={provider} value={provider}>
                {provider}
              </option>
            ))}
          </select>
        </div>

        <div className="filter-group">
          <label htmlFor="provider-multi">{t("filter.providers")}</label>
          <select
            id="provider-multi"
            multiple
            value={filter.providers}
            onChange={(e) =>
              handleProvidersChange(Array.from(e.currentTarget.selectedOptions, (option) => option.value))
            }
          >
            {(filterOptions?.providers ?? []).map((provider) => (
              <option key={provider} value={provider}>
                {provider}
              </option>
            ))}
          </select>
        </div>

        <div className="filter-group">
          <label htmlFor="stream-filter">{t("filter.streamMode")}</label>
          <select
            id="stream-filter"
            value={filter.isStream === undefined ? "" : String(filter.isStream)}
            onChange={(e) =>
              handleStreamChange(e.target.value === "" ? undefined : e.target.value === "true")
            }
          >
            <option value="">{t("filter.allModes")}</option>
            <option value="true">{t("mode.stream")}</option>
            <option value="false">{t("mode.nonStream")}</option>
          </select>
        </div>

        <div className="filter-group">
          <label htmlFor="status-filter">{t("filter.status")}</label>
          <select
            id="status-filter"
            value={filter.status}
            onChange={(e) => handleStatusChange(e.target.value)}
          >
            <option value="">{t("filter.allStatuses")}</option>
            <option value="success">{t("status.success")}</option>
            <option value="completed">{t("status.completed")}</option>
            <option value="error_*">{t("status.errorAny")}</option>
            <option value="incomplete">{t("status.incomplete")}</option>
            {(filterOptions?.statuses ?? [])
              .filter((status) => !["success", "completed", "error_*", "incomplete"].includes(status))
              .map((status) => (
                <option key={status} value={status}>
                  {status}
                </option>
              ))}
          </select>
        </div>

        <div className="filter-group">
          <label htmlFor="sort-by">{t("filter.sortBy")}</label>
          <select id="sort-by" value={filter.sortBy} onChange={(e) => handleSortChange(e.target.value as RequestSortBy)}>
            <option value="startedAt">{t("sort.startedAt")}</option>
            <option value="tokens">{t("sort.tokens")}</option>
            <option value="duration">{t("sort.duration")}</option>
            <option value="model">{t("sort.model")}</option>
          </select>
        </div>

        <div className="filter-group">
          <label htmlFor="sort-dir">{t("filter.sortDir")}</label>
          <select id="sort-dir" value={filter.sortDir} onChange={(e) => handleSortDirChange(e.target.value as RequestSortDir)}>
            <option value="desc">{t("sort.desc")}</option>
            <option value="asc">{t("sort.asc")}</option>
          </select>
        </div>

        <div className="filter-group wide">
          <label htmlFor="model-filter">{t("filter.model")}</label>
          <input
            id="model-filter"
            type="text"
            placeholder={t("filter.modelPlaceholder")}
            value={filter.model}
            onChange={(e) => handleModelChange(e.target.value)}
          />
        </div>

        <div className="filter-group wide">
          <label htmlFor="model-query">{t("filter.modelQuery")}</label>
          <input
            id="model-query"
            type="text"
            placeholder={t("filter.modelQueryPlaceholder")}
            value={filter.modelQuery}
            onChange={(e) => handleModelQueryChange(e.target.value)}
          />
        </div>
      </section>

      {data ? (
        <section className="request-summary-panel" aria-label={t("requests.summary.title")}>
          <div className="request-summary-header">
            <span>{t("requests.summary.title")}</span>
          </div>
          <div className="request-summary-grid">
            <div className="request-summary-item">
              <span>{t("requests.summary.records")}</span>
              <strong>{formatNumber(data.total)}</strong>
            </div>
            <div className="request-summary-item">
              <span>{t("requests.summary.input")}</span>
              <strong>{formatNumber(data.totalInputTokens)}</strong>
            </div>
            <div className="request-summary-item highlight">
              <span>{t("requests.summary.cached")}</span>
              <strong>{formatNumber(data.totalCachedInputTokens)}</strong>
            </div>
            <div className="request-summary-item">
              <span>{t("requests.summary.output")}</span>
              <strong>{formatNumber(data.totalOutputTokens)}</strong>
            </div>
            <div className="request-summary-item">
              <span>{t("requests.summary.reasoning")}</span>
              <strong>{formatNumber(data.totalReasoningTokens)}</strong>
            </div>
          </div>
        </section>
      ) : null}

      <section className="requests-table-panel">
        {data?.records.length ? (
          <>
            <div className="table-container">
              <table className="data-table">
                <thead>
                  <tr>
                    <th>{t("th.model_request_id")}</th>
                    <th>{t("th.provider")}</th>
                    <th>{t("th.mode")}</th>
                    <th>{t("th.input")}</th>
                    <th>{t("th.output")}</th>
                    <th>{t("th.cached")}</th>
                    <th>{t("th.reasoning")}</th>
                    <th>{t("th.ttft")}</th>
                    <th>{t("th.duration")}</th>
                    <th>{t("th.status")}</th>
                    <th>{t("th.started")}</th>
                  </tr>
                </thead>
                <tbody>
                  {data.records.map((record) => renderRequestRow(record, handleSelectRequest, t))}
                </tbody>
              </table>
            </div>

            <div className="pagination">
              <button
                type="button"
                className="secondary"
                onClick={handlePrevPage}
                disabled={isPending || !data || data.offset === 0}
              >
                {t("pagination.previous")}
              </button>
              <span className="pagination-info">
                {t("pagination.info",
                  String(data.offset + 1),
                  String(Math.min(data.offset + data.limit, data.total)),
                  String(data.total))}
              </span>
              <button
                type="button"
                className="secondary"
                onClick={handleNextPage}
                disabled={isPending || !data || data.offset + data.limit >= data.total}
              >
                {t("pagination.next")}
              </button>
            </div>
          </>
        ) : (
          <p className="empty">
            {isPending ? t("loading.requests") : t("requests.empty")}
          </p>
        )}
      </section>

      {isLoadingDetail ? (
        <div className="drawer-overlay">
          <div className="drawer">
            <div className="drawer-content">
              <p className="empty">{t("loading.requestDetail")}</p>
            </div>
          </div>
        </div>
      ) : null}

      <RequestDetailDrawer detail={selectedDetail} onClose={handleCloseDetail} t={t} />
    </div>
  );
}

export default Requests;
