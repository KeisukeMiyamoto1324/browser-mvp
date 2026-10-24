import { invoke } from "@tauri-apps/api/core";
import {
  type ChangeEvent,
  type FormEvent,
  type JSX,
  useEffect,
  useRef,
  useState,
} from "react";

type NavigationState = {
  url: string;
  nativeToolbar: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
  loading: boolean;
  progress: number;
};

// ---------------------------------------------------------
// Control the native content WebView from the local toolbar.
// ---------------------------------------------------------
export default function App(): JSX.Element {
  const [address, setAddress] = useState("https://duckduckgo.com/");
  const [error, setError] = useState("");
  const editing = useRef(false);
  const [navigation, setNavigation] = useState<NavigationState>({
    url: "",
    nativeToolbar: false,
    canGoBack: false,
    canGoForward: false,
    loading: false,
    progress: 0,
  });

  useEffect((): (() => void) => {
    let active = true;
    function resizeContent(): void {
      void invoke("resize_content", {
        viewportHeight: window.innerHeight,
      }).catch((cause: unknown): void => {
        if (active) setError(String(cause));
      });
    }
    async function updateAddress(): Promise<void> {
      try {
        const state = await invoke<NavigationState>("browser_navigation");
        if (!active) return;
        setNavigation(state);
        if (!editing.current && state.url) setAddress(state.url);
      } catch (cause: unknown) {
        if (active) setError(String(cause));
      }
    }
    resizeContent();
    window.addEventListener("resize", resizeContent);
    let timer: number;
    async function pollNavigation(): Promise<void> {
      await updateAddress();
      if (active)
        timer = window.setTimeout((): void => {
          void pollNavigation();
        }, 100);
    }
    void pollNavigation();
    return (): void => {
      active = false;
      window.removeEventListener("resize", resizeContent);
      window.clearInterval(timer);
    };
  }, []);

  async function navigate(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const submittedAddress = new FormData(event.currentTarget).get("address");
    setError("");
    try {
      await invoke("navigate", { address: String(submittedAddress ?? "") });
      editing.current = false;
      (document.activeElement as HTMLElement | null)?.blur();
    } catch (cause: unknown) {
      setError(String(cause));
    }
  }

  async function controlNavigation(
    action: "back" | "forward" | "reload" | "stop",
  ): Promise<void> {
    setError("");
    try {
      setNavigation(
        await invoke<NavigationState>("browser_navigation", { action }),
      );
    } catch (cause: unknown) {
      setError(String(cause));
    }
  }

  if (navigation.nativeToolbar) return <main />;

  return (
    <main>
      <nav className="toolbar" aria-label="Browser controls">
        <button
          type="button"
          disabled={!navigation.canGoBack}
          aria-label="Back"
          title="Back"
          onClick={(): void => {
            void controlNavigation("back");
          }}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="m14 6-6 6 6 6" />
          </svg>
        </button>
        <button
          type="button"
          disabled={!navigation.canGoForward}
          aria-label="Forward"
          title="Forward"
          onClick={(): void => {
            void controlNavigation("forward");
          }}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="m10 6 6 6-6 6" />
          </svg>
        </button>
        <form
          onSubmit={(event: FormEvent<HTMLFormElement>): void => {
            void navigate(event);
          }}
        >
          <input
            type="text"
            name="address"
            aria-label="Search or enter a URL"
            placeholder="Search or enter a URL"
            value={address}
            autoCapitalize="off"
            autoComplete="off"
            spellCheck={false}
            onFocus={(): void => {
              editing.current = true;
            }}
            onBlur={(): void => {
              editing.current = false;
            }}
            onChange={(event: ChangeEvent<HTMLInputElement>): void => {
              setAddress(event.target.value);
            }}
          />
          <button
            className="reload"
            type="button"
            aria-label={
              navigation.loading ? "Stop loading" : "Reload this page"
            }
            title={navigation.loading ? "Stop loading" : "Reload this page"}
            onClick={(): void => {
              void controlNavigation(navigation.loading ? "stop" : "reload");
            }}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              {navigation.loading ? (
                <path d="m7 7 10 10M17 7 7 17" />
              ) : (
                <path d="M19 10a7 7 0 1 0 0 5M19 4v6h-6" />
              )}
            </svg>
          </button>
          {navigation.loading && (
            <span
              className="load-progress"
              style={{ width: `${navigation.progress * 100}%` }}
            />
          )}
        </form>
        <span className="error" role="status">
          {error}
        </span>
      </nav>
    </main>
  );
}
