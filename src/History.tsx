import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type ChangeEvent, type JSX, useEffect, useState } from "react";

type Visit = { url: string; title: string; visitedAt: number };
type ClosedWindow = {
  id: string;
  closed_at: number;
  tabs: { url: string; title: string }[];
  is_window: boolean;
};

// ---------------------------------------------------------
// Keep the history surface in the trusted local WebView.
// Remote page content is hidden while this surface is open.
// ---------------------------------------------------------
export default function History(): JSX.Element | null {
  const [visible, setVisible] = useState(false);
  const [query, setQuery] = useState("");
  const [visits, setVisits] = useState<Visit[]>([]);
  const [closed, setClosed] = useState<ClosedWindow[]>([]);
  const [clearDialog, setClearDialog] = useState(false);
  const [range, setRange] = useState("all");
  const [error, setError] = useState("");

  useEffect((): (() => void) => {
    const subscription = listen("show-history", (): void => {
      setVisible(true);
    });
    return (): void => {
      void subscription.then((unlisten): void => {
        unlisten();
      });
    };
  }, []);

  useEffect((): (() => void) => {
    let active = true;
    void invoke("history_visible", { visible }).catch(
      (cause: unknown): void => {
        if (active) setError(String(cause));
      },
    );
    if (visible) {
      void Promise.all([
        invoke<Visit[]>("history_query", { query }),
        invoke<ClosedWindow[]>("history_recent"),
      ])
        .then(([history, recent]): void => {
          if (!active) return;
          setVisits(history);
          setClosed(recent);
        })
        .catch((cause: unknown): void => {
          if (active) setError(String(cause));
        });
    }
    return (): void => {
      active = false;
    };
  }, [visible, query]);

  async function openVisit(url: string): Promise<void> {
    try {
      await invoke("navigate", { address: url });
      setVisible(false);
    } catch (cause: unknown) {
      setError(String(cause));
    }
  }

  async function clearHistory(): Promise<void> {
    const hours = range === "hour" ? 1 : 24;
    const since = range === "all" ? 0 : Date.now() - hours * 3600000;
    try {
      await invoke("history_clear", { since });
      const [history, recent] = await Promise.all([
        invoke<Visit[]>("history_query", { query }),
        invoke<ClosedWindow[]>("history_recent"),
      ]);
      setVisits(history);
      setClosed(recent);
      setClearDialog(false);
    } catch (cause: unknown) {
      setError(String(cause));
    }
  }

  useEffect((): (() => void) => {
    function keyDown(event: KeyboardEvent): void {
      if (event.key !== "Escape" || !visible) return;
      event.preventDefault();
      if (clearDialog) setClearDialog(false);
      else setVisible(false);
    }
    window.addEventListener("keydown", keyDown);
    return (): void => {
      window.removeEventListener("keydown", keyDown);
    };
  }, [visible, clearDialog]);

  if (!visible) return null;
  let previousDay = "";
  return (
    <section className="history-page" aria-label="Browsing history">
      <header>
        <h1>History</h1>
        <button
          type="button"
          onClick={(): void => {
            setVisible(false);
          }}
        >
          Done
        </button>
      </header>
      <div className="history-controls">
        <input
          aria-label="Search history"
          placeholder="Search History"
          value={query}
          onChange={(event: ChangeEvent<HTMLInputElement>): void => {
            setQuery(event.target.value);
          }}
        />
        <button
          type="button"
          onClick={(): void => {
            setClearDialog(true);
          }}
        >
          Clear History…
        </button>
      </div>
      {closed.length > 0 && (
        <details className="recently-closed">
          <summary>Recently Closed</summary>
          {closed.map(
            (session): JSX.Element => (
              <button
                type="button"
                key={session.id}
                onClick={(): void => {
                  void invoke("history_reopen", { id: session.id })
                    .then((): void => {
                      setVisible(false);
                    })
                    .catch((cause: unknown): void => {
                      setError(String(cause));
                    });
                }}
              >
                {session.is_window
                  ? `Window with ${session.tabs.length} tabs`
                  : session.tabs[0]?.title || session.tabs[0]?.url || "Tab"}
              </button>
            ),
          )}
        </details>
      )}
      <p role="status">{error}</p>
      {visits.length === 0 && (
        <p className="history-empty">
          {query ? "No matching history" : "No browsing history"}
        </p>
      )}
      <div className="history-entries">
        {visits.map((visit): JSX.Element => {
          const date = new Date(visit.visitedAt);
          const day = date.toLocaleDateString(undefined, {
            weekday: "long",
            year: "numeric",
            month: "long",
            day: "numeric",
          });
          const heading = day !== previousDay;
          previousDay = day;
          return (
            <div key={`${visit.visitedAt}-${visit.url}`}>
              {heading && <h2>{day}</h2>}
              <button
                type="button"
                className="history-entry"
                onClick={(): void => {
                  void openVisit(visit.url);
                }}
              >
                <time dateTime={date.toISOString()}>
                  {date.toLocaleTimeString(undefined, {
                    hour: "numeric",
                    minute: "2-digit",
                  })}
                </time>
                <span>
                  <strong>{visit.title || visit.url}</strong>
                  <small>{visit.url}</small>
                </span>
              </button>
            </div>
          );
        })}
      </div>
      {clearDialog && (
        <div className="history-scrim">
          <section
            role="dialog"
            aria-modal="true"
            aria-labelledby="clear-history-title"
            className="history-dialog"
          >
            <h2 id="clear-history-title">Clear History</h2>
            <p>
              Remove visited pages and recently closed tabs and windows. Open
              tabs and website data will stay.
            </p>
            <label>
              Clear{" "}
              <select
                value={range}
                onChange={(event: ChangeEvent<HTMLSelectElement>): void => {
                  setRange(event.target.value);
                }}
              >
                <option value="hour">the last hour</option>
                <option value="day">the last 24 hours</option>
                <option value="all">all history</option>
              </select>
            </label>
            <footer>
              <button
                type="button"
                onClick={(): void => {
                  setClearDialog(false);
                }}
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={(): void => {
                  void clearHistory();
                }}
              >
                Clear History
              </button>
            </footer>
          </section>
        </div>
      )}
    </section>
  );
}
