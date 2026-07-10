import { useState } from "react";
import { CheckCircle2, Loader2, PlayCircle, Plug, ShieldAlert, XCircle } from "lucide-react";
import { Button } from "../components/Button";
import { Field, Input } from "../components/Field";
import { isTauri } from "../api";
import { useConnection } from "../store/connection";

export function Connect() {
  const { connect, test, connecting, remembered } = useConnection();

  const [host, setHost] = useState(remembered.host);
  const [port, setPort] = useState(String(remembered.port));
  const [password, setPassword] = useState(remembered.password);
  const [testing, setTesting] = useState(false);
  const [demoing, setDemoing] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; message?: string } | null>(null);

  const conn = () => ({ host: host.trim(), port: Number(port) || 8212, password });

  const onTest = async () => {
    setTesting(true);
    setResult(null);
    try {
      setResult(await test(conn()));
    } finally {
      setTesting(false);
    }
  };

  const onConnect = async () => {
    setResult(null);
    const res = await connect(conn());
    if (!res.ok) setResult(res);
  };

  const onDemo = async () => {
    setDemoing(true);
    setResult(null);
    try {
      await connect({ host: "demo", port: 8212, password: "demo" }, { demo: true });
    } finally {
      setDemoing(false);
    }
  };

  return (
    <div className="connect">
      <div className="card card--pad connect__card">
        <div className="connect__brand">
          <div className="connect__logo" aria-hidden />
          <div>
            <h1>Palworld Server Manager</h1>
            <p>Connect to your dedicated server’s REST API</p>
          </div>
        </div>

        <form
          className="connect__form"
          onSubmit={(e) => {
            e.preventDefault();
            onConnect();
          }}
        >
          <div className="connect__ports">
            <Field label="Host">
              <Input value={host} onChange={(e) => setHost(e.target.value)} placeholder="localhost or LAN IP" mono autoFocus />
            </Field>
            <Field label="Port">
              <Input value={port} onChange={(e) => setPort(e.target.value)} placeholder="8212" mono inputMode="numeric" />
            </Field>
          </div>

          <Field label="Admin password" hint="Your server’s AdminPassword (username is always “admin”).">
            <Input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="••••••••"
              mono
            />
          </Field>

          {result && (
            <div className={`connect__result ${result.ok ? "connect__result--ok" : "connect__result--err"}`}>
              {result.ok ? <CheckCircle2 size={15} /> : <XCircle size={15} />}
              {result.message ?? (result.ok ? "Connection successful." : "Connection failed.")}
            </div>
          )}

          <div className="row" style={{ gap: 10, marginTop: 2 }}>
            <Button type="button" variant="ghost" onClick={onTest} disabled={testing || connecting}>
              {testing ? <Loader2 size={16} className="spin" /> : null}
              Test
            </Button>
            <Button type="submit" variant="primary" block loading={connecting && !demoing}>
              <Plug size={16} />
              Connect
            </Button>
          </div>
        </form>

        <button className="connect__demo" onClick={onDemo} disabled={connecting}>
          {demoing ? <Loader2 size={14} className="spin" /> : <PlayCircle size={14} />}
          Explore with demo data
        </button>

        <div className="connect__note">
          <ShieldAlert size={14} />
          <span>
            The REST API is intended for LAN use — avoid exposing it directly to the internet.
            {!isTauri() && " In the browser, requests route through the local dev proxy; the desktop app connects natively."}
          </span>
        </div>
      </div>
    </div>
  );
}
