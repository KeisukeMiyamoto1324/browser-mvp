import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type FormEvent, type JSX, useEffect, useRef, useState } from "react";

type Preferences = {
  hideFavorites: boolean;
  hideRecent: boolean;
  background: boolean;
};

type Destination = {
  title: string;
  url: string;
  tab?: string;
  window?: string;
};

type Snapshot = {
  preferences: Preferences;
  openTabs: Destination[];
  favorites: Destination[];
  recent: Destination[];
};

// ---------------------------------------------------------
// Keep the local page independent of any remote document.
// All destinations come from this application's own records.
// ---------------------------------------------------------
export default function StartPage(): JSX.Element {
  const [snapshot, setSnapshot] = useState<Snapshot>({
    preferences: { hideFavorites: false, hideRecent: false, background: false },
    openTabs: [],
    favorites: [],
    recent: [],
  });
  const [customizing, setCustomizing] = useState(false);
  const favoriteDialog = useRef<HTMLDialogElement>(null);
  const [error, setError] = useState("");

  useEffect((): (() => void) => {
    let active = true;
    function refresh(): void {
      void invoke<Snapshot>("get_start_page")
        .then((value: Snapshot): void => {
          if (active) setSnapshot(value);
        })
        .catch((cause: unknown): void => {
          if (active) setError(String(cause));
        });
    }
    refresh();
    window.addEventListener("focus", refresh);
    const subscriptions = [
      listen("saved-pages-changed", refresh),
      listen("start-page-changed", refresh),
      listen("history-changed", refresh),
    ];
    return (): void => {
      active = false;
      window.removeEventListener("focus", refresh);
      for (const subscription of subscriptions) {
        void subscription
          .then((unlisten): void => {
            unlisten();
          })
          .catch((): void => {});
      }
    };
  }, []);

  async function open(destination: Destination): Promise<void> {
    setError("");
    try {
      if (destination.tab && destination.window) {
        await invoke("select_suggested_tab", {
          tab: destination.tab,
          window: destination.window,
        });
        return;
      }
      await invoke("navigate", { address: destination.url });
    } catch (cause: unknown) {
      setError(String(cause));
    }
  }

  async function customize(key: keyof Preferences): Promise<void> {
    const preferences = {
      ...snapshot.preferences,
      [key]: !snapshot.preferences[key],
    };
    try {
      await invoke("set_start_page", { preferences });
      setSnapshot({ ...snapshot, preferences });
      setError("");
    } catch (cause: unknown) {
      setError(String(cause));
    }
  }

  function section(
    title: string,
    entries: Destination[],
    empty: string,
  ): JSX.Element {
    const uniqueEntries = Array.from(
      new Map(
        entries.map((entry: Destination) => [entry.tab ?? entry.url, entry]),
      ).values(),
    );
    return (
      <section className="start-section" aria-label={title}>
        <div className="start-section-heading">
          <h2>{title}</h2>
          {title === "Favorites" && (
            <button
              type="button"
              aria-label="Add Favorite"
              onClick={(): void => {
                favoriteDialog.current?.showModal();
              }}
            >
              +
            </button>
          )}
        </div>
        {entries.length === 0 ? (
          <p className="start-empty">{empty}</p>
        ) : (
          <div className="start-tiles">
            {uniqueEntries.slice(0, 12).map(
              (entry: Destination): JSX.Element => (
                <button
                  type="button"
                  className="start-tile"
                  key={`${entry.tab ?? ""}${entry.url}`}
                  title={entry.url}
                  onClick={(): void => {
                    void open(entry);
                  }}
                >
                  <span className="site-initial" aria-hidden="true">
                    {entry.title.slice(0, 1).toUpperCase()}
                  </span>
                  <span className="site-title">{entry.title}</span>
                </button>
              ),
            )}
          </div>
        )}
      </section>
    );
  }

  return (
    <main
      className={`start-page${snapshot.preferences.background ? " start-background" : ""}`}
      onKeyDown={(event): void => {
        if (event.key === "Escape") setCustomizing(false);
      }}
    >
      <div className="start-content">
        <h1>Start Page</h1>
        <form
          className="start-search"
          onSubmit={(event: FormEvent<HTMLFormElement>): void => {
            event.preventDefault();
            const address = new FormData(event.currentTarget).get("address");
            void open({ title: "", url: String(address ?? "") });
          }}
        >
          <input
            name="address"
            aria-label="Search or enter a URL"
            placeholder="Search or enter website name"
            autoComplete="off"
            autoCapitalize="off"
            spellCheck={false}
            required
          />
          <button type="submit" aria-label="Go">
            →
          </button>
        </form>
        {!snapshot.preferences.hideFavorites &&
          section(
            "Favorites",
            snapshot.favorites ?? [],
            "Your favorite bookmarks will appear here.",
          )}
        {!snapshot.preferences.hideRecent &&
          section(
            "Recently Visited",
            snapshot.recent ?? [],
            "Pages you visit in this browser will appear here.",
          )}
        {snapshot.openTabs.length > 0 &&
          section("Open Tabs", snapshot.openTabs, "")}
        {error && (
          <p role="alert" className="start-error">
            {error}
          </p>
        )}
      </div>
      <dialog className="favorite-dialog" ref={favoriteDialog}>
        <form
          onSubmit={(event: FormEvent<HTMLFormElement>): void => {
            event.preventDefault();
            const form = event.currentTarget;
            const values = new FormData(form);
            void invoke("edit_saved_pages", {
              edit: {
                kind: "bookmark",
                parentId: null,
                title: String(values.get("title") ?? ""),
                url: String(values.get("url") ?? ""),
              },
            })
              .then((): void => {
                favoriteDialog.current?.close();
                form.reset();
                setError("");
              })
              .catch((cause: unknown): void => {
                setError(String(cause));
                favoriteDialog.current?.close();
              });
          }}
        >
          <h2>Add Favorite</h2>
          <label htmlFor="favorite-title">Name</label>
          <input id="favorite-title" name="title" required />
          <label htmlFor="favorite-url">Address</label>
          <input
            id="favorite-url"
            name="url"
            type="url"
            placeholder="https://"
            required
          />
          <div className="favorite-actions">
            <button
              type="button"
              onClick={(): void => {
                favoriteDialog.current?.close();
              }}
            >
              Cancel
            </button>
            <button type="submit">Add</button>
          </div>
        </form>
      </dialog>
      <div className="start-customize">
        {customizing && (
          <fieldset className="start-options" aria-label="Customize Start Page">
            <h2>Customize Start Page</h2>
            {(
              [
                [
                  "hideFavorites",
                  "Favorites",
                  !snapshot.preferences.hideFavorites,
                ],
                [
                  "hideRecent",
                  "Recently Visited",
                  !snapshot.preferences.hideRecent,
                ],
                ["background", "Background", snapshot.preferences.background],
              ] as const
            ).map(
              ([key, label, checked]): JSX.Element => (
                <label key={key}>
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={(): void => {
                      void customize(key);
                    }}
                  />
                  {label}
                </label>
              ),
            )}
          </fieldset>
        )}
        <button
          type="button"
          aria-label="Customize Start Page"
          aria-expanded={customizing}
          onClick={(): void => {
            setCustomizing(!customizing);
          }}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M4 7h16M4 17h16M8 4v6M16 14v6" />
          </svg>
        </button>
      </div>
    </main>
  );
}
