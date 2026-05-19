import { useEffect, useState, useTransition } from "react";
import {
  listFilteredRequests,
  getRequestDetail,
  type RequestFilterInput,
  type RequestRecordListItem,
  type RequestRecordDetail,
  type PaginatedRequestRecords,
} from "./desktop";
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

function renderRequestRow(
  request: RequestRecordListItem,
  onSelect: (id: string) => void,
) {
  return (
    <tr key={request.id} onClick={() => onSelect(request.id)} className="request-row">
      <td>
        <div className="primary-cell">
          <strong>{request.model ?? "Unknown model"}</strong>
          <span>{request.requestId ?? request.id}</span>
        </div>
      </td>
      <td>
        <span className={`provider-badge ${request.provider}`}>{request.provider}</span>
      </td>
      <td>{request.isStream ? "Stream" : "Non-stream"}</td>
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

function RequestDetailDrawer({
  detail,
  onClose,
}: {
  detail: RequestRecordDetail | null;
  onClose: () => void;
}) {
  if (!detail) {
    return null;
  }

  return (
    <div className="drawer-overlay" onClick={onClose}>
      <div className="drawer" onClick={(e) => e.stopPropagation()}>
        <div className="drawer-header">
          <h2>Request Detail</h2>
          <button type="button" className="close-btn" onClick={onClose}>
            ×
          </button>
        </div>

        <div className="drawer-content">
          <section className="detail-section">
            <h3>Basic Information</h3>
            <dl className="detail-grid">
              <div>
                <dt>Provider</dt>
                <dd>
                  <span className={`provider-badge ${detail.provider}`}>{detail.provider}</span>
                </dd>
              </div>
              <div>
                <dt>Source Mode</dt>
                <dd>{detail.sourceMode}</dd>
              </div>
              <div>
                <dt>Model</dt>
                <dd className="mono">{detail.model ?? "N/A"}</dd>
              </div>
              <div>
                <dt>Stream Mode</dt>
                <dd>{detail.isStream ? "Yes" : "No"}</dd>
              </div>
              <div>
                <dt>Status</dt>
                <dd>{detail.status}</dd>
              </div>
              <div>
                <dt>Request ID</dt>
                <dd className="mono">{detail.requestId ?? "N/A"}</dd>
              </div>
              <div>
                <dt>Session ID</dt>
                <dd className="mono">{detail.sessionId ?? "N/A"}</dd>
              </div>
              <div>
                <dt>Working Directory</dt>
                <dd className="mono">{detail.cwd ?? "N/A"}</dd>
              </div>
              <div>
                <dt>Entrypoint</dt>
                <dd className="mono">{detail.entrypoint ?? "N/A"}</dd>
              </div>
            </dl>
          </section>

          <section className="detail-section">
            <h3>Token Usage</h3>
            <div className="stats-grid">
              <div className="stat-card">
                <span>Input Tokens</span>
                <strong>{formatNumber(detail.inputTokens)}</strong>
              </div>
              <div className="stat-card">
                <span>Output Tokens</span>
                <strong>{formatNumber(detail.outputTokens)}</strong>
              </div>
              <div className="stat-card">
                <span>Cached Input</span>
                <strong>{formatNumber(detail.cachedInputTokens)}</strong>
              </div>
              <div className="stat-card">
                <span>Reasoning</span>
                <strong>{formatNumber(detail.reasoningTokens)}</strong>
              </div>
              <div className="stat-card">
                <span>Total Tokens</span>
                <strong>{formatNumber(detail.inputTokens + detail.outputTokens)}</strong>
              </div>
            </div>
          </section>

          <section className="detail-section">
            <h3>Latency</h3>
            <div className="stats-grid">
              <div className="stat-card">
                <span>TTFT</span>
                <strong>{formatMs(detail.ttftMs)}</strong>
              </div>
              <div className="stat-card">
                <span>Duration</span>
                <strong>{formatMs(detail.durationMs)}</strong>
              </div>
            </div>
          </section>

          <section className="detail-section">
            <h3>Timing</h3>
            <dl className="detail-grid">
              <div>
                <dt>Started At</dt>
                <dd>{formatDateTime(detail.startedAt)}</dd>
              </div>
              <div>
                <dt>Finished At</dt>
                <dd>{formatDateTime(detail.finishedAt)}</dd>
              </div>
            </dl>
          </section>

          {detail.requestSummaryJson ? (
            <section className="detail-section">
              <h3>Request Summary</h3>
              <pre className="json-block">
                {JSON.stringify(JSON.parse(detail.requestSummaryJson), null, 2)}
              </pre>
            </section>
          ) : null}

          {detail.responseSummaryJson ? (
            <section className="detail-section">
              <h3>Response Summary</h3>
              <pre className="json-block">
                {JSON.stringify(JSON.parse(detail.responseSummaryJson), null, 2)}
              </pre>
            </section>
          ) : null}

          {detail.errorText ? (
            <section className="detail-section error-section">
              <h3>Error</h3>
              <pre className="error-block">{detail.errorText}</pre>
            </section>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function Requests() {
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
          refreshError instanceof Error ? refreshError.message : "Failed to load requests.",
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
        detailError instanceof Error ? detailError.message : "Failed to load request detail.",
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
        <h1>Request Records</h1>
        <button type="button" onClick={refresh} disabled={isPending}>
          Refresh
        </button>
      </div>

      {error ? <section className="notice error">{error}</section> : null}

      <section className="filters-panel">
        <div className="filter-group">
          <label htmlFor="provider-filter">Provider</label>
          <select
            id="provider-filter"
            value={filter.provider ?? ""}
            onChange={(e) => handleProviderChange(e.target.value || undefined)}
          >
            <option value="">All Providers</option>
            <option value="claude_code">Claude Code</option>
            <option value="codex">Codex</option>
            <option value="openai_compat">OpenAI Compat</option>
            <option value="anthropic_compat">Anthropic Compat</option>
          </select>
        </div>

        <div className="filter-group">
          <label htmlFor="stream-filter">Stream Mode</label>
          <select
            id="stream-filter"
            value={filter.isStream === undefined ? "" : String(filter.isStream)}
            onChange={(e) =>
              handleStreamChange(e.target.value === "" ? undefined : e.target.value === "true")
            }
          >
            <option value="">All Modes</option>
            <option value="true">Stream</option>
            <option value="false">Non-stream</option>
          </select>
        </div>

        <div className="filter-group">
          <label htmlFor="model-filter">Model</label>
          <input
            id="model-filter"
            type="text"
            placeholder="Filter by model name..."
            value={filter.model ?? ""}
            onChange={(e) => handleModelChange(e.target.value || undefined)}
          />
        </div>
      </section>

      <section className="requests-table-panel">
        {data?.records.length ? (
          <>
            <div className="table-shell">
              <table className="request-table">
                <thead>
                  <tr>
                    <th>Model / Request ID</th>
                    <th>Provider</th>
                    <th>Mode</th>
                    <th>Input</th>
                    <th>Output</th>
                    <th>Cached</th>
                    <th>Reasoning</th>
                    <th>TTFT</th>
                    <th>Duration</th>
                    <th>Status</th>
                    <th>Started</th>
                  </tr>
                </thead>
                <tbody>
                  {data.records.map((record) => renderRequestRow(record, handleSelectRequest))}
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
                Previous
              </button>
              <span className="pagination-info">
                {data.offset + 1}-{Math.min(data.offset + data.limit, data.total)} of {data.total}
              </span>
              <button
                type="button"
                className="secondary"
                onClick={handleNextPage}
                disabled={isPending || !data || data.offset + data.limit >= data.total}
              >
                Next
              </button>
            </div>
          </>
        ) : (
          <p className="empty">
            {isPending ? "Loading..." : "No requests found matching the current filters."}
          </p>
        )}
      </section>

      {isLoadingDetail ? (
        <div className="drawer-overlay">
          <div className="drawer">
            <div className="drawer-content">
              <p className="empty">Loading request detail...</p>
            </div>
          </div>
        </div>
      ) : null}

      <RequestDetailDrawer detail={selectedDetail} onClose={handleCloseDetail} />
    </div>
  );
}

export default Requests;
