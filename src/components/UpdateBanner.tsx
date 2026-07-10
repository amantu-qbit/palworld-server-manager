import { ArrowUpCircle, Loader2, X } from "lucide-react";
import { useUpdater } from "../store/updater";

function firstLine(s: string): string {
  const line = s.split("\n").find((l) => l.trim().length > 0)?.trim() ?? "";
  return line.length > 110 ? line.slice(0, 110) + "…" : line;
}

export function UpdateBanner() {
  const u = useUpdater();
  if (!u.available || u.dismissed) return null;

  const busy = u.status === "downloading" || u.status === "installing";
  const pct = Math.round(u.progress * 100);

  return (
    <div className="updbar" role="status">
      <span className="updbar__ic">
        <ArrowUpCircle size={16} />
      </span>
      <div className="updbar__msg">
        <b>Update available</b>
        <span>
          Version {u.version} is ready to install{u.notes ? ` · ${firstLine(u.notes)}` : ""}.
        </span>
      </div>
      {busy ? (
        <span className="updbar__status">
          <Loader2 size={14} className="spin" />
          {u.status === "installing" ? "Installing…" : `Downloading ${pct}%`}
        </span>
      ) : (
        <div className="updbar__actions">
          <button className="btn btn--primary btn--sm" onClick={() => void u.install()}>
            Update now
          </button>
          <button className="updbar__x" onClick={u.dismiss} aria-label="Dismiss">
            <X size={15} />
          </button>
        </div>
      )}
    </div>
  );
}
