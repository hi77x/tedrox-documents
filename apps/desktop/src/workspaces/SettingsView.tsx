import { useStore } from "../state";
import { IconCheck, IconKeyboard, IconShield } from "../design/icons";

const SHORTCUTS: { keys: string; label: string }[] = [
  { keys: "Ctrl + K", label: "Command palette" },
  { keys: "Ctrl + O", label: "Open file" },
  { keys: "Ctrl + N", label: "New document" },
  { keys: "Ctrl + S", label: "Save" },
  { keys: "Ctrl + Shift + S", label: "Save as" },
  { keys: "Ctrl + W", label: "Close tab" },
  { keys: "Ctrl + F", label: "Find" },
  { keys: "Ctrl + B / I / U", label: "Bold, italic, underline" },
  { keys: "F2", label: "Edit the active cell" },
  { keys: "Ctrl + Arrow", label: "Jump in the grid" },
];

export function SettingsView() {
  const { settings, setSettings, clearRecent, recent, pushToast } = useStore();

  return (
    <div className="home" style={{ padding: 24 }}>
      <div className="home-inner" style={{ maxWidth: 760 }}>
        <h1 style={{ fontSize: 20 }}>Settings</h1>
        <p className="home-lede">
          Preferences are stored on this machine only. Documents are never uploaded.
        </p>

        <section className="panel" style={{ marginBottom: 16 }}>
          <p className="panel-title">General</p>
          <div className="col">
            <label className="field">
              <span>Theme</span>
              <select className="select" value={settings.theme} onChange={(event) => setSettings({ theme: event.target.value as typeof settings.theme })}>
                <option value="system">Follow the system</option>
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </select>
            </label>
            <label className="field">
              <span>Language</span>
              <select className="select" value={settings.language} onChange={(event) => setSettings({ language: event.target.value as typeof settings.language })}>
                <option value="en">English</option>
                <option value="ru">Русский</option>
              </select>
            </label>
            <label className="field">
              <span>Default zoom for documents (%)</span>
              <input
                className="number-input"
                type="number"
                min={50}
                max={200}
                value={settings.defaultZoom}
                onChange={(event) => setSettings({ defaultZoom: Number(event.target.value) })}
              />
            </label>
            <label className="checkbox">
              <input type="checkbox" checked={settings.autosave} onChange={(event) => setSettings({ autosave: event.target.checked })} />
              Remind me to save when closing a modified tab
            </label>
            <label className="checkbox">
              <input type="checkbox" checked={settings.restoreSession} onChange={(event) => setSettings({ restoreSession: event.target.checked })} />
              Restore open files on the next launch
            </label>
            <label className="checkbox">
              <input type="checkbox" checked={settings.spellcheck} onChange={(event) => setSettings({ spellcheck: event.target.checked })} />
              Check spelling while typing
            </label>
          </div>
        </section>

        <section className="panel" style={{ marginBottom: 16 }}>
          <p className="panel-title">
            <IconKeyboard /> Keyboard
          </p>
          <div className="table-shell">
            <table>
              <tbody>
                {SHORTCUTS.map((item) => (
                  <tr key={item.keys}>
                    <td className="mono" style={{ width: 190, color: "var(--text)" }}>
                      {item.keys}
                    </td>
                    <td>{item.label}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        <section className="panel" style={{ marginBottom: 16 }}>
          <p className="panel-title">
            <IconShield /> Privacy
          </p>
          <p className="small muted">
            TEDROX Documents processes files on this device. There is no telemetry, no analytics, no crash
            reporting and no account. Logs contain operation ids, file types, sizes and durations — never
            document contents or passwords. Outputs are written to a temporary sibling file and validated before
            replacing anything.
          </p>
        </section>

        <section className="panel" style={{ marginBottom: 16 }}>
          <p className="panel-title">Local data</p>
          <p className="small muted">{recent.length} recent entries stored in this browser profile.</p>
          <button
            className="btn"
            onClick={() => {
              clearRecent();
              pushToast({ kind: "success", message: "Recent list cleared" });
            }}
          >
            Clear recent files
          </button>
        </section>

        <section className="panel">
          <p className="panel-title">About</p>
          <p className="small muted">
            TEDROX Documents · MIT licensed · local-first document suite. Compatibility levels are derived from
            automated tests; the format matrix never claims more than the test suite proves.
          </p>
          <p className="small muted">
            <IconCheck /> No file leaves this machine unless you explicitly export it.
          </p>
        </section>
      </div>
    </div>
  );
}
