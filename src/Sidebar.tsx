import { invoke } from "@tauri-apps/api/core";
import {
  type Event as AppEvent,
  listen,
  type UnlistenFn,
} from "@tauri-apps/api/event";
import {
  type ChangeEvent,
  type FormEvent,
  type JSX,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
  useEffect,
  useRef,
  useState,
} from "react";
import "./sidebar.css";

type Bookmark = {
  id: number;
  parentId: number | null;
  title: string;
  url: string | null;
};
type ReadingItem = { id: number; title: string; url: string; read: boolean };
type SavedPages = { bookmarks: Bookmark[]; readingList: ReadingItem[] };
type Editor = {
  kind: "bookmark" | "reading";
  id: number | null;
  title: string;
  url: string | null;
  parentId: number | null;
  read: boolean;
};
type Row = { item: Bookmark | ReadingItem; depth: number; folder: boolean };
type Props = { url: string; title: string; nativeToolbar: boolean };

// ---------------------------------------------------------
// Keep saved content shared while sidebar layout stays local.
// All persistent edits are validated by the owning Rust store.
// ---------------------------------------------------------
export default function Sidebar({
  url,
  title,
  nativeToolbar,
}: Props): JSX.Element {
  const [open, setOpen] = useState(false);
  const [width, setWidth] = useState(280);
  const [section, setSection] = useState<"bookmarks" | "reading">("bookmarks");
  const [pages, setPages] = useState<SavedPages>({
    bookmarks: [],
    readingList: [],
  });
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const [selected, setSelected] = useState<number | null>(null);
  const [context, setContext] = useState<{ x: number; y: number } | null>(null);
  const [editor, setEditor] = useState<Editor | null>(null);
  const [error, setError] = useState("");
  const [unreadOnly, setUnreadOnly] = useState(false);
  const current = useRef({ url, title });
  const panel = useRef<HTMLElement>(null);
  const editorName = useRef<HTMLInputElement>(null);
  const editorKey = editor ? `${editor.kind}-${editor.id ?? "new"}` : "";
  useEffect((): void => {
    if (editorKey) {
      editorName.current?.focus();
      editorName.current?.select();
    }
  }, [editorKey]);
  useEffect((): void => {
    if (context)
      panel.current
        ?.querySelector<HTMLButtonElement>("[role=menu] button")
        ?.focus();
  }, [context]);
  current.current = { url, title };

  useEffect((): (() => void) => {
    let active = true;
    const refresh = (): void => {
      void invoke<SavedPages>("get_saved_pages")
        .then((data: SavedPages): void => {
          if (active) setPages(data);
        })
        .catch((cause: unknown): void => {
          if (active) setError(String(cause));
        });
    };
    refresh();
    const changed = listen("saved-pages-changed", refresh);
    const action = listen<string>(
      "sidebar-action",
      (event: AppEvent<string>): void => {
        const name = event.payload;
        setContext(null);
        setEditor(null);
        if (name === "toggle-sidebar") {
          setOpen((value: boolean): boolean => !value);
          return;
        }
        setOpen(true);
        if (name === "show-bookmarks") setSection("bookmarks");
        if (name === "show-reading-list") setSection("reading");
        if (name !== "add-bookmark" && name !== "add-reading") return;
        const page = current.current;
        if (!/^https?:\/\//.test(page.url)) {
          setError("Open a web page before saving it.");
          return;
        }
        const reading = name === "add-reading";
        setSection(reading ? "reading" : "bookmarks");
        setEditor({
          kind: reading ? "reading" : "bookmark",
          id: null,
          title: page.title || page.url,
          url: page.url,
          parentId: null,
          read: false,
        });
      },
    );
    return (): void => {
      active = false;
      void changed.then((stop: UnlistenFn): void => stop());
      void action.then((stop: UnlistenFn): void => stop());
    };
  }, []);

  useEffect((): void => {
    document.documentElement.style.setProperty(
      "--sidebar-width",
      `${open ? width : 0}px`,
    );
    void invoke("resize_content", {
      viewportHeight: window.innerHeight,
      sidebarWidth: open ? width : 0,
    }).catch((cause: unknown): void => setError(String(cause)));
  }, [open, width]);

  const editingItem = editor !== null;
  useEffect((): void => {
    if (open && !editingItem)
      panel.current?.querySelector<HTMLElement>("[role=tree]")?.focus();
  }, [open, editingItem]);

  useEffect((): (() => void) => {
    const constrainWidth = (): void =>
      setWidth((value: number): number =>
        Math.min(value, Math.max(220, window.innerWidth - 160)),
      );
    constrainWidth();
    window.addEventListener("resize", constrainWidth);
    return (): void => window.removeEventListener("resize", constrainWidth);
  }, []);

  const rows: Row[] = [];
  if (section === "reading") {
    for (const item of pages.readingList) {
      if (!unreadOnly || !item.read)
        rows.push({ item, depth: 0, folder: false });
    }
  } else {
    const append = (parent: number | null, depth: number): void => {
      for (const item of pages.bookmarks.filter(
        (bookmark: Bookmark): boolean => bookmark.parentId === parent,
      )) {
        rows.push({ item, depth, folder: item.url === null });
        if (item.url === null && expanded.has(item.id))
          append(item.id, depth + 1);
      }
    };
    append(null, 0);
  }
  const selectedRow = rows.find(
    (row: Row): boolean => row.item.id === selected,
  );

  async function mutate(edit: Record<string, unknown>): Promise<void> {
    setError("");
    try {
      setPages(await invoke<SavedPages>("edit_saved_pages", { edit }));
      setEditor(null);
    } catch (cause: unknown) {
      setError(String(cause));
    }
  }

  function editItem(): void {
    if (!selectedRow) return;
    const item = selectedRow.item;
    setEditor({
      kind: section === "reading" ? "reading" : "bookmark",
      id: item.id,
      title: item.title,
      url: item.url,
      parentId: "parentId" in item ? item.parentId : null,
      read: "read" in item && item.read,
    });
  }

  function addItem(folder: boolean): void {
    if (!folder && !/^https?:\/\//.test(url)) {
      setError("Open a web page before saving it.");
      return;
    }
    setEditor({
      kind: section === "reading" ? "reading" : "bookmark",
      id: null,
      title: folder ? "New Folder" : title || url,
      url: folder ? null : url,
      parentId: selectedRow?.folder ? selectedRow.item.id : null,
      read: false,
    });
  }

  function activate(row: Row): void {
    setSelected(row.item.id);
    if (row.folder) {
      setExpanded((value: Set<number>): Set<number> => {
        const next = new Set(value);
        if (next.has(row.item.id)) next.delete(row.item.id);
        else next.add(row.item.id);
        return next;
      });
      return;
    }
    void invoke("navigate", { address: row.item.url }).catch(
      (cause: unknown): void => setError(String(cause)),
    );
    if ("read" in row.item && !row.item.read)
      void mutate({ kind: "reading", ...row.item, read: true });
  }

  function keyboard(event: KeyboardEvent<HTMLElement>): void {
    if (event.key === "Escape" && context) {
      event.preventDefault();
      setContext(null);
      panel.current?.querySelector<HTMLElement>("[role=tree]")?.focus();
      return;
    }
    if (event.key === "Escape" && editor) {
      event.preventDefault();
      setEditor(null);
      return;
    }
    if ((event.target as HTMLElement).closest("input,select,form,button"))
      return;
    const index = rows.findIndex(
      (row: Row): boolean => row.item.id === selected,
    );
    if (event.key === "Escape") {
      setOpen(false);
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      setSelected(
        rows[
          Math.max(
            0,
            Math.min(
              rows.length - 1,
              index + (event.key === "ArrowDown" ? 1 : -1),
            ),
          )
        ]?.item.id ?? null,
      );
    }
    if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      setSelected(
        rows[event.key === "Home" ? 0 : rows.length - 1]?.item.id ?? null,
      );
    }
    if (!selectedRow) return;
    if (event.key === "F2") {
      event.preventDefault();
      editItem();
    }
    if (event.key === "Enter") {
      event.preventDefault();
      activate(selectedRow);
    }
    if (
      event.key === "ArrowRight" &&
      selectedRow.folder &&
      !expanded.has(selectedRow.item.id)
    )
      activate(selectedRow);
    if (
      event.key === "ArrowLeft" &&
      selectedRow.folder &&
      expanded.has(selectedRow.item.id)
    )
      activate(selectedRow);
    if (event.key === "Backspace" || event.key === "Delete") {
      event.preventDefault();
      void mutate({
        kind: section === "reading" ? "deleteReading" : "deleteBookmark",
        id: selected,
      });
    }
  }

  function resize(event: PointerEvent<HTMLHRElement>): void {
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    if (editor) void mutate({ ...editor });
  }

  return (
    <>
      {!nativeToolbar && (
        <button
          className="sidebar-toggle"
          type="button"
          aria-label="Show Sidebar"
          aria-expanded={open}
          onClick={(): void => setOpen(!open)}
        >
          ☰
        </button>
      )}
      <aside
        ref={panel}
        className={`saved-sidebar ${open ? "is-open" : ""}`}
        style={{ width: open ? width : 0, top: nativeToolbar ? 0 : 64 }}
        aria-label="Sidebar"
        aria-hidden={!open}
        tabIndex={open ? 0 : -1}
        onKeyDown={keyboard}
      >
        <header className="sidebar-heading">
          <strong>Sidebar</strong>
          <button
            type="button"
            aria-label="Hide Sidebar"
            onClick={(): void => setOpen(false)}
          >
            ×
          </button>
        </header>
        <div
          className="sidebar-sections"
          role="tablist"
          aria-label="Saved pages"
        >
          <button
            type="button"
            role="tab"
            aria-selected={section === "bookmarks"}
            onClick={(): void => {
              setSection("bookmarks");
              setSelected(null);
            }}
          >
            Bookmarks
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={section === "reading"}
            onClick={(): void => {
              setSection("reading");
              setSelected(null);
            }}
          >
            Reading List
          </button>
        </div>
        {section === "reading" && (
          <label className="unread-filter">
            <input
              type="checkbox"
              checked={unreadOnly}
              onChange={(): void => setUnreadOnly(!unreadOnly)}
            />
            Show unread only
          </label>
        )}
        <div
          className="saved-rows"
          role="tree"
          tabIndex={open ? 0 : -1}
          aria-label={section === "reading" ? "Reading List" : "Bookmarks"}
          aria-activedescendant={selected ? `saved-${selected}` : undefined}
        >
          {rows.map(
            (row: Row): JSX.Element => (
              <div
                key={row.item.id}
                id={`saved-${row.item.id}`}
                role="treeitem"
                tabIndex={-1}
                onKeyDown={(event: KeyboardEvent<HTMLDivElement>): void => {
                  if (event.key === "F2") editItem();
                }}
                aria-level={row.depth + 1}
                aria-selected={selected === row.item.id}
                aria-expanded={
                  row.folder ? expanded.has(row.item.id) : undefined
                }
                className={`saved-row ${selected === row.item.id ? "selected" : ""}`}
                style={{ paddingLeft: 10 + row.depth * 16 }}
                onClick={(): void => {
                  setSelected(row.item.id);
                  setContext(null);
                  if (!row.folder) activate(row);
                  panel.current
                    ?.querySelector<HTMLElement>("[role=tree]")
                    ?.focus();
                }}
                onDoubleClick={(): void => {
                  if (row.folder) activate(row);
                }}
                onContextMenu={(event: MouseEvent<HTMLDivElement>): void => {
                  event.preventDefault();
                  setSelected(row.item.id);
                  setContext({
                    x: Math.min(event.clientX, width - 168),
                    y: Math.min(event.clientY, window.innerHeight - 160),
                  });
                }}
              >
                <button
                  className="saved-open"
                  type="button"
                  aria-label={
                    row.folder
                      ? `${expanded.has(row.item.id) ? "Collapse" : "Expand"} ${row.item.title}`
                      : `Open ${row.item.title}`
                  }
                  onClick={(event: MouseEvent<HTMLButtonElement>): void => {
                    event.stopPropagation();
                    activate(row);
                  }}
                >
                  {row.folder
                    ? expanded.has(row.item.id)
                      ? "⌄"
                      : "›"
                    : "read" in row.item && row.item.read
                      ? "○"
                      : "◉"}
                </button>
                <span>
                  <span className="saved-title">{row.item.title}</span>
                  {section === "reading" && (
                    <span className="saved-host">
                      {new URL(row.item.url ?? "https://localhost").hostname}
                    </span>
                  )}
                </span>
              </div>
            ),
          )}
          {rows.length === 0 && (
            <p className="saved-empty">
              {section === "reading"
                ? "Pages you save to read later appear here."
                : "Save pages and organize them in folders."}
            </p>
          )}
        </div>
        <div className="saved-actions">
          <button type="button" onClick={(): void => addItem(false)}>
            Add Page
          </button>
          {section === "bookmarks" && (
            <button type="button" onClick={(): void => addItem(true)}>
              New Folder
            </button>
          )}
          <button type="button" disabled={!selectedRow} onClick={editItem}>
            Edit
          </button>
          <button
            type="button"
            disabled={!selectedRow}
            onClick={(): void => {
              void mutate({
                kind:
                  section === "reading" ? "deleteReading" : "deleteBookmark",
                id: selected,
              });
            }}
          >
            Delete
          </button>
        </div>
        {editor && (
          <form className="saved-editor" onSubmit={submit}>
            <strong>
              {editor.id ? "Edit" : "Add"}{" "}
              {editor.url === null
                ? "Folder"
                : editor.kind === "reading"
                  ? "Reading Item"
                  : "Bookmark"}
            </strong>
            <label>
              Name
              <input
                ref={editorName}
                required
                value={editor.title}
                onChange={(event: ChangeEvent<HTMLInputElement>): void =>
                  setEditor({ ...editor, title: event.target.value })
                }
              />
            </label>
            {editor.url !== null && (
              <label>
                Address
                <input
                  required
                  type="url"
                  value={editor.url}
                  onChange={(event: ChangeEvent<HTMLInputElement>): void =>
                    setEditor({ ...editor, url: event.target.value })
                  }
                />
              </label>
            )}
            {editor.kind === "bookmark" && (
              <label>
                Folder
                <select
                  value={editor.parentId ?? ""}
                  onChange={(event: ChangeEvent<HTMLSelectElement>): void =>
                    setEditor({
                      ...editor,
                      parentId: event.target.value
                        ? Number(event.target.value)
                        : null,
                    })
                  }
                >
                  <option value="">Favorites</option>
                  {pages.bookmarks
                    .filter(
                      (item: Bookmark): boolean =>
                        item.url === null && item.id !== editor.id,
                    )
                    .map(
                      (item: Bookmark): JSX.Element => (
                        <option key={item.id} value={item.id}>
                          {item.title}
                        </option>
                      ),
                    )}
                </select>
              </label>
            )}
            {editor.kind === "reading" && (
              <label className="read-toggle">
                <input
                  type="checkbox"
                  checked={editor.read}
                  onChange={(): void =>
                    setEditor({ ...editor, read: !editor.read })
                  }
                />
                Read
              </label>
            )}
            <div>
              <button type="button" onClick={(): void => setEditor(null)}>
                Cancel
              </button>
              <button type="submit">Save</button>
            </div>
          </form>
        )}
        {error && (
          <p className="saved-error" role="alert">
            {error}
          </p>
        )}
        {context && selectedRow && (
          <div
            className="saved-context"
            role="menu"
            aria-label="Saved page actions"
            style={{
              left: Math.max(8, context.x),
              top: Math.max(8, context.y),
            }}
            onKeyDown={(event: KeyboardEvent<HTMLDivElement>): void => {
              if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
              event.preventDefault();
              event.stopPropagation();
              const buttons = Array.from(
                event.currentTarget.querySelectorAll<HTMLButtonElement>(
                  "button",
                ),
              );
              const index = buttons.indexOf(
                document.activeElement as HTMLButtonElement,
              );
              buttons[
                (index + (event.key === "ArrowDown" ? 1 : buttons.length - 1)) %
                  buttons.length
              ]?.focus();
            }}
          >
            <button
              type="button"
              role="menuitem"
              onClick={(): void => {
                activate(selectedRow);
                setContext(null);
              }}
            >
              Open
            </button>
            <button
              type="button"
              role="menuitem"
              onClick={(): void => {
                editItem();
                setContext(null);
              }}
            >
              Edit…
            </button>
            {"read" in selectedRow.item && (
              <button
                type="button"
                role="menuitem"
                onClick={(): void => {
                  void mutate({
                    kind: "reading",
                    ...selectedRow.item,
                    read:
                      "read" in selectedRow.item
                        ? !selectedRow.item.read
                        : false,
                  });
                  setContext(null);
                }}
              >
                {selectedRow.item.read ? "Mark as Unread" : "Mark as Read"}
              </button>
            )}
            <button
              type="button"
              role="menuitem"
              onClick={(): void => {
                void mutate({
                  kind:
                    section === "reading" ? "deleteReading" : "deleteBookmark",
                  id: selected,
                });
                setContext(null);
              }}
            >
              Delete
            </button>
          </div>
        )}
        <hr
          className="sidebar-resizer"
          aria-label="Sidebar width"
          aria-orientation="vertical"
          aria-valuemin={220}
          aria-valuemax={Math.max(220, Math.min(480, window.innerWidth - 160))}
          aria-valuenow={width}
          tabIndex={open ? 0 : -1}
          onPointerDown={resize}
          onPointerMove={(event: PointerEvent<HTMLHRElement>): void => {
            if (event.currentTarget.hasPointerCapture(event.pointerId))
              setWidth(
                Math.max(
                  220,
                  Math.min(480, window.innerWidth - 160, event.clientX),
                ),
              );
          }}
          onKeyDown={(event: KeyboardEvent<HTMLHRElement>): void => {
            if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
              event.preventDefault();
              event.stopPropagation();
              setWidth(
                Math.max(
                  220,
                  Math.min(
                    480,
                    window.innerWidth - 160,
                    width + (event.key === "ArrowLeft" ? -10 : 10),
                  ),
                ),
              );
            }
          }}
        />
      </aside>
    </>
  );
}
