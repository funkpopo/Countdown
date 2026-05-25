import { useEffect, useState, useTransition } from "react";
import {
  listFilteredRequests,
  getRequestDetail,
  type RequestFilterInput,
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

  return date.toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
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
              <pre className="json-block">
                {JSON.stringify(JSON.parse(detail.requestSummaryJson), null, 2)}
              </pre>
            </section>
          ) : null}

          {detail.responseSummaryJson ? (
            <section className="detail-section">
              <h3>{t("detail.responseSummary")}</h3>
              <pre className="json-block">
                {JSON.stringify(JSON.parse(detail.responseSummaryJson), null, 2)}
              </pre>
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
  const [filter, setFilter] = useState<RequestFilterInput>({
    limit: 50,
    offset: 0,
  });
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

  const handleModelChange = (model: string | undefined) => {
    setFilter((prev) => ({ ...prev, model, offset: 0 }));
  };

  const handleStreamChange = (isStream: boolean | undefined) => {
    setFilter((prev) => ({ ...prev, isStream, offset: 0 }));
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
        <button type="button" onClick={refresh} disabled={isPending}>
          {t("requests.refresh")}
        </button>
      </div>

      {error ? <section className="notice error">{error}</section> : null}

      <section className="filters-panel">
        <div className="filter-group">
          <label htmlFor="provider-filter">{t("filter.provider")}</label>
          <select
            id="provider-filter"
            value={filter.provider ?? ""}
            onChange={(e) => handleProviderChange(e.target.value || undefined)}
          >
            <option value="">{t("filter.allProviders")}</option>
            <option value="claude_code">Claude Code</option>
            <option value="codex">Codex</option>
            <option value="openai_compat">OpenAI Compat</option>
            <option value="anthropic_compat">Anthropic Compat</option>
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
          <label htmlFor="model-filter">{t("filter.model")}</label>
          <input
            id="model-filter"
            type="text"
            placeholder={t("filter.modelPlaceholder")}
            value={filter.model ?? ""}
            onChange={(e) => handleModelChange(e.target.value || undefined)}
          />
        </div>
      </section>

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
