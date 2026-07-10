import "./banmanager.css";
import { useState } from "react";
import { ShieldBan } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { Button } from "../components/Button";
import { Field, Input } from "../components/Field";
import { EmptyState } from "../components/EmptyState";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { ConfirmSpec } from "../components/ConfirmDialog";
import { DataTable } from "../components/DataTable";
import type { Column } from "../components/DataTable";
import { api } from "../api";
import { useToast } from "../hooks/useToast";
import { addBan, removeBan, useBans } from "../store/banRecord";
import type { BanRecord } from "../store/banRecord";

const DEFAULT_REASON = "Banned by admin";

export function BanManager() {
  const bans = useBans();
  const toast = useToast();
  const [confirm, setConfirm] = useState<ConfirmSpec | null>(null);
  const [banId, setBanId] = useState("");
  const [reason, setReason] = useState(DEFAULT_REASON);
  const [unbanId, setUnbanId] = useState("");

  const unban = async (userid: string) => {
    const r = await api.unban(userid);
    if (r.ok) {
      removeBan(userid);
      toast.success("Player unbanned");
    } else {
      toast.error("Unban failed", r.message);
    }
  };

  const columns: Column<BanRecord>[] = [
    { key: "userid", header: "User ID", render: (r) => <span className="bm-userid">{r.userid}</span> },
    { key: "reason", header: "Reason", render: (r) => r.reason },
    {
      key: "ts",
      header: "When",
      render: (r) => <span className="bm-when">{new Date(r.ts).toLocaleString()}</span>,
    },
    {
      key: "action",
      header: "",
      align: "right",
      render: (r) => (
        <Button variant="ghost" size="sm" onClick={() => unban(r.userid)}>
          Unban
        </Button>
      ),
    },
  ];

  return (
    <>
      <TopBar breadcrumb="Control" title="Ban Manager" showLive={false} />
      <div className="page">
        <div className="card card--pad bm-note">
          The REST API has no endpoint to list existing bans, so this shows bans issued through this app. You
          can still unban any user ID below.
        </div>

        <div className="grid-2">
          <div className="card card--pad">
            <div className="section-head">
              <h2>Ban a player</h2>
            </div>
            <div className="bm-form">
              <Field label="User ID">
                <Input value={banId} onChange={(e) => setBanId(e.target.value)} mono placeholder="steam_…" />
              </Field>
              <Field label="Reason">
                <Input value={reason} onChange={(e) => setReason(e.target.value)} />
              </Field>
              <Button
                variant="danger"
                block
                disabled={!banId.trim()}
                onClick={() =>
                  setConfirm({
                    danger: true,
                    requireText: banId,
                    title: "Ban this user?",
                    body: "User ID: " + banId,
                    confirmText: "Ban",
                    onConfirm: async () => {
                      const r = await api.ban(banId, reason);
                      if (r.ok) {
                        addBan(banId, reason);
                        toast.success("Player banned");
                      } else {
                        toast.error("Ban failed", r.message);
                      }
                      setBanId("");
                      setReason(DEFAULT_REASON);
                    },
                  })
                }
              >
                Ban
              </Button>
            </div>
          </div>

          <div className="card card--pad">
            <div className="section-head">
              <h2>Unban a player</h2>
            </div>
            <div className="bm-form">
              <Field label="User ID" hint="Works even if the ban wasn’t issued here.">
                <Input
                  value={unbanId}
                  onChange={(e) => setUnbanId(e.target.value)}
                  mono
                  placeholder="steam_…"
                />
              </Field>
              <Button
                block
                disabled={!unbanId.trim()}
                onClick={async () => {
                  await unban(unbanId);
                  setUnbanId("");
                }}
              >
                Unban
              </Button>
            </div>
          </div>
        </div>

        <div className="section-head">
          <h2>Recorded bans</h2>
        </div>
        {bans.length === 0 ? (
          <EmptyState
            icon={ShieldBan}
            title="No bans recorded"
            detail="Bans you issue here will be listed for quick unbanning."
          />
        ) : (
          <DataTable columns={columns} rows={bans} rowKey={(r) => r.userid} />
        )}
      </div>
      <ConfirmDialog spec={confirm} onClose={() => setConfirm(null)} />
    </>
  );
}
