import "./console.css";
import { useState } from "react";
import { Activity, Megaphone, Power, PowerOff, Save } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { Button } from "../components/Button";
import { Field, Input } from "../components/Field";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { ConfirmSpec } from "../components/ConfirmDialog";
import { api } from "../api";
import { useToast } from "../hooks/useToast";
import { logActivity, useActivityLog } from "../store/activityLog";
import type { LogEntry } from "../store/activityLog";

const DOT: Record<LogEntry["kind"], string> = {
  info: "var(--accent)",
  success: "var(--good)",
  error: "var(--bad)",
};

export function Console() {
  const toast = useToast();
  const log = useActivityLog();
  const [confirm, setConfirm] = useState<ConfirmSpec | null>(null);

  const [announceMsg, setAnnounceMsg] = useState("");
  const [broadcasting, setBroadcasting] = useState(false);
  const [savingWorld, setSavingWorld] = useState(false);
  const [wait, setWait] = useState("30");
  const [shutdownMsg, setShutdownMsg] = useState("");

  async function onAnnounce() {
    const msg = announceMsg.trim();
    if (!msg) return;
    setBroadcasting(true);
    try {
      const r = await api.announce(msg);
      if (r.ok) {
        logActivity("success", "Announced: " + msg);
        toast.success("Announced");
        setAnnounceMsg("");
      } else {
        logActivity("error", r.message || "Announce failed");
        toast.error("Announce failed", r.message);
      }
    } finally {
      setBroadcasting(false);
    }
  }

  async function onSave() {
    setSavingWorld(true);
    try {
      const r = await api.saveWorld();
      if (r.ok) {
        logActivity("success", "World saved");
        toast.success("World saved", r.message);
      } else {
        logActivity("error", r.message || "Save failed");
        toast.error("Save failed", r.message);
      }
    } finally {
      setSavingWorld(false);
    }
  }

  function onShutdown() {
    const secs = Math.max(0, Math.floor(Number(wait) || 0));
    const msg = shutdownMsg.trim();
    setConfirm({
      danger: true,
      title: "Schedule shutdown?",
      body: (
        <>
          The server will shut down in <b>{secs}</b> second{secs === 1 ? "" : "s"}
          {msg ? (
            <>
              {" "}with the message “{msg}”.
            </>
          ) : (
            "."
          )}
        </>
      ),
      confirmText: "Shutdown",
      onConfirm: async () => {
        const r = await api.shutdown(secs, msg);
        if (r.ok) {
          logActivity("success", `Shutdown scheduled in ${secs}s`);
          toast.success("Shutdown scheduled", r.message);
        } else {
          logActivity("error", r.message || "Shutdown failed");
          toast.error("Shutdown failed", r.message);
        }
      },
    });
  }

  function onForceStop() {
    setConfirm({
      danger: true,
      requireText: "STOP",
      title: "Force stop the server?",
      body: "This stops the server immediately without a countdown.",
      confirmText: "Force stop",
      onConfirm: async () => {
        const r = await api.forceStop();
        if (r.ok) {
          logActivity("success", "Server force stopped");
          toast.success("Server stopped", r.message);
        } else {
          logActivity("error", r.message || "Force stop failed");
          toast.error("Force stop failed", r.message);
        }
      },
    });
  }

  return (
    <>
      <TopBar breadcrumb="Control" title="Console" showLive={false} />
      <div className="page">
        <div className="grid-2">
          {/* Announce */}
          <div className="card card--pad console-card">
            <div className="card__glow" />
            <div className="console-card__head">
              <span className="console-card__ic">
                <Megaphone size={16} />
              </span>
              <div>
                <div className="console-card__title">Announce</div>
                <div className="console-card__sub">Broadcast a message to every player</div>
              </div>
            </div>
            <textarea
              className="input"
              placeholder="Server restarting soon — wrap up what you’re doing."
              value={announceMsg}
              onChange={(e) => setAnnounceMsg(e.target.value)}
            />
            <div className="console-card__foot">
              <Button
                variant="primary"
                onClick={onAnnounce}
                loading={broadcasting}
                disabled={!announceMsg.trim()}
              >
                <Megaphone size={15} /> Broadcast
              </Button>
            </div>
          </div>

          {/* Save World */}
          <div className="card card--pad console-card">
            <div className="console-card__head">
              <span className="console-card__ic">
                <Save size={16} />
              </span>
              <div>
                <div className="console-card__title">Save World</div>
                <div className="console-card__sub">Force a save checkpoint now</div>
              </div>
            </div>
            <p className="console-card__copy">
              Writes the current world state to disk immediately, so progress is safe before a
              restart or maintenance.
            </p>
            <div className="console-card__foot">
              <Button onClick={onSave} loading={savingWorld}>
                <Save size={15} /> Save now
              </Button>
            </div>
          </div>

          {/* Shutdown */}
          <div className="card card--pad console-card">
            <div className="console-card__head">
              <span className="console-card__ic">
                <Power size={16} />
              </span>
              <div>
                <div className="console-card__title">Shutdown</div>
                <div className="console-card__sub">Stop the server after a countdown</div>
              </div>
            </div>
            <div className="console-fields">
              <Field label="Wait (seconds)">
                <Input
                  type="number"
                  min={0}
                  mono
                  value={wait}
                  onChange={(e) => setWait(e.target.value)}
                />
              </Field>
              <Field label="Message">
                <Input
                  placeholder="Scheduled restart"
                  value={shutdownMsg}
                  onChange={(e) => setShutdownMsg(e.target.value)}
                />
              </Field>
            </div>
            <div className="console-card__foot">
              <Button variant="danger" onClick={onShutdown}>
                <Power size={15} /> Schedule shutdown
              </Button>
            </div>
          </div>

          {/* Force Stop */}
          <div className="card card--pad console-card">
            <div className="console-card__head">
              <span className="console-card__ic console-card__ic--danger">
                <PowerOff size={16} />
              </span>
              <div>
                <div className="console-card__title">Force Stop</div>
                <div className="console-card__sub">Kill the server process instantly</div>
              </div>
            </div>
            <p className="console-card__copy console-card__copy--warn">
              No countdown and no automatic save — anything since the last save is lost. Use only
              when the server is unresponsive.
            </p>
            <div className="console-card__foot">
              <Button variant="danger" onClick={onForceStop}>
                <PowerOff size={15} /> Force stop
              </Button>
            </div>
          </div>
        </div>

        {/* Activity log */}
        <div className="card card--pad console-card console-activity">
          <div className="console-card__head">
            <span className="console-card__ic">
              <Activity size={16} />
            </span>
            <div>
              <div className="console-card__title">Activity</div>
              <div className="console-card__sub">Recent commands and their results</div>
            </div>
          </div>
          {log.length === 0 ? (
            <div className="console-log__empty">No activity yet — commands you run show up here.</div>
          ) : (
            <ul className="console-log">
              {log.map((e) => (
                <li key={e.id} className="console-log__row">
                  <span className="console-log__time">{new Date(e.ts).toLocaleTimeString()}</span>
                  <span className="console-log__dot" style={{ color: DOT[e.kind] }} />
                  <span className="console-log__text">{e.text}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
      <ConfirmDialog spec={confirm} onClose={() => setConfirm(null)} />
    </>
  );
}
