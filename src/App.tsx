import { invoke } from "@tauri-apps/api/core";
import { type ChangeEvent, type FormEvent, type JSX, useState } from "react";
import tauriLogo from "./assets/tauri.svg";

// ---------------------------------------------------------
// Render the greeting form and call the Rust command.
// ---------------------------------------------------------
export default function App(): JSX.Element {
  const [name, setName] = useState("");
  const [message, setMessage] = useState("");

  async function greet(): Promise<void> {
    try {
      setMessage(await invoke<string>("greet", { name }));
    } catch (error: unknown) {
      setMessage(`Failed to greet: ${String(error)}`);
    }
  }

  return (
    <main className="container">
      <h1>Welcome to Tauri</h1>
      <div className="row">
        <a href="https://tauri.app" target="_blank" rel="noreferrer">
          <img src={tauriLogo} className="logo tauri" alt="Tauri logo" />
        </a>
      </div>
      <p>Click on the Tauri logo to learn more about the framework</p>
      <form
        className="row"
        onSubmit={(event: FormEvent<HTMLFormElement>): void => {
          event.preventDefault();
          void greet();
        }}
      >
        <input
          id="greet-input"
          aria-label="Name"
          placeholder="Enter a name..."
          value={name}
          onChange={(event: ChangeEvent<HTMLInputElement>): void =>
            setName(event.target.value)
          }
        />
        <button type="submit">Greet</button>
      </form>
      <p id="greet-msg" aria-live="polite">
        {message}
      </p>
    </main>
  );
}
