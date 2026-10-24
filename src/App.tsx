import { invoke } from "@tauri-apps/api/core";
import {
  type ChangeEvent,
  type FormEvent,
  type JSX,
  useEffect,
  useRef,
  useState,
} from "react";

// ---------------------------------------------------------
// Control the native content WebView from the local toolbar.
// ---------------------------------------------------------
export default function App(): JSX.Element {
  const [address, setAddress] = useState("https://duckduckgo.com/");
  const [error, setError] = useState("");
  const editing = useRef(false);

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
        const url = await invoke<string>("browser_url");
        if (active && !editing.current) setAddress(url);
      } catch (cause: unknown) {
        if (active) setError(String(cause));
      }
    }
    resizeContent();
    window.addEventListener("resize", resizeContent);
    void updateAddress();
    const timer = window.setInterval((): void => {
      void updateAddress();
    }, 500);
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

  async function navigateHistory(direction: "back" | "forward"): Promise<void> {
    setError("");
    try {
      await invoke("navigate_history", { direction });
    } catch (cause: unknown) {
      setError(String(cause));
    }
  }

  return (
    <main>
      <nav className="toolbar" aria-label="Browser controls">
        <button
          type="button"
          aria-label="Back"
          title="Back"
          onClick={(): void => {
            void navigateHistory("back");
          }}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="m14 6-6 6 6 6" />
          </svg>
        </button>
        <button
          type="button"
          aria-label="Forward"
          title="Forward"
          onClick={(): void => {
            void navigateHistory("forward");
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
        </form>
        <span className="error" role="status">
          {error}
        </span>
      </nav>
    </main>
  );
}
