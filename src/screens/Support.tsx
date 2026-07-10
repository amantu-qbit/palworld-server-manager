import "./support.css";
import { useEffect, useState } from "react";
import { QRCodeSVG } from "qrcode.react";
import { ArrowUpCircle, CheckCircle2, Coffee, Copy, Heart, Loader2, TriangleAlert } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { Button } from "../components/Button";
import { useToast } from "../hooks/useToast";
import { useUpdater } from "../store/updater";
import { openExternal } from "../lib/external";
import { isTauri } from "../api";

const BMC_URL = "https://buymeacoffee.com/amantukhan";

interface Coin {
  key: string;
  glyph: string;
  name: string;
  network: string;
  address: string;
}

const COINS: Coin[] = [
  { key: "btc", glyph: "₿", name: "Bitcoin", network: "BTC", address: "1LQYtBNQsFq7myLAjFNrxrWPaZ7WEGLkFD" },
  { key: "eth", glyph: "Ξ", name: "Ethereum", network: "ERC-20", address: "0x8e97b63448652124e386884772efdd216b3964af" },
  { key: "usdt", glyph: "₮", name: "Tether USDT", network: "Tron · TRC-20", address: "TCY5Ds14UgZfaLX3seWzDRWGmS4bFMjyan" },
];

export function Support() {
  const toast = useToast();
  const u = useUpdater();
  const [appVersion, setAppVersion] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    let alive = true;
    import("@tauri-apps/api/app")
      .then((m) => m.getVersion())
      .then((v) => {
        if (alive) setAppVersion(v);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  const copy = (coin: Coin) => {
    navigator.clipboard.writeText(coin.address);
    toast.success("Address copied", `${coin.name} (${coin.network})`);
  };

  return (
    <>
      <TopBar breadcrumb="About" title="Support" showLive={false} />
      <div className="page">
        {/* Intro + Buy me a coffee */}
        <div className="card card--pad sup-hero">
          <div className="card__glow" />
          <div className="sup-hero__ic">
            <Heart size={20} />
          </div>
          <div className="sup-hero__body">
            <h2>Support the project</h2>
            <p>
              Palworld Server Manager is free and open-source under the MIT license. If it saves you
              time running your server, consider chipping in — it directly funds development and is
              hugely appreciated, but never required.
            </p>
          </div>
          <Button variant="primary" className="sup-bmc" onClick={() => void openExternal(BMC_URL)}>
            <Coffee size={16} /> Buy me a coffee
          </Button>
        </div>

        {/* Crypto */}
        <div className="eyebrow sup-label">Or send crypto</div>
        <div className="sup-coins">
          {COINS.map((coin) => (
            <div key={coin.key} className="card card--pad donate">
              <div className="donate__head">
                <span className={`donate__glyph donate__glyph--${coin.key}`} aria-hidden>
                  {coin.glyph}
                </span>
                <div className="donate__id">
                  <b>{coin.name}</b>
                  <span className="chip chip--accent">{coin.network}</span>
                </div>
              </div>
              <div className="donate__qr">
                <QRCodeSVG
                  value={coin.address}
                  size={132}
                  bgColor="#ffffff"
                  fgColor="#0a0a0b"
                  level="M"
                  marginSize={2}
                />
              </div>
              <button className="donate__addr" onClick={() => copy(coin)} title="Copy address">
                <span className="donate__addrtext">{coin.address}</span>
                <Copy size={14} />
              </button>
            </div>
          ))}
        </div>

        {/* Version + updates */}
        <div className="card card--pad sup-about">
          <div className="sup-about__row">
            <div>
              <div className="eyebrow">Version</div>
              <div className="sup-about__ver mono">{appVersion ? `v${appVersion}` : "Desktop app"}</div>
            </div>
            <div className="sup-about__upd">
              <UpdateStatus />
              <Button
                onClick={() => void u.check(true)}
                loading={u.status === "checking"}
                disabled={!u.supported}
              >
                <ArrowUpCircle size={15} /> Check for updates
              </Button>
            </div>
          </div>
          {!u.supported && (
            <p className="sup-about__hint">Automatic updates run in the installed desktop app.</p>
          )}
        </div>
      </div>
    </>
  );
}

function UpdateStatus() {
  const u = useUpdater();
  if (u.status === "checking") {
    return (
      <span className="sup-upd sup-upd--dim">
        <Loader2 size={14} className="spin" /> Checking…
      </span>
    );
  }
  if (u.available && u.version) {
    return (
      <span className="sup-upd sup-upd--accent">
        <ArrowUpCircle size={14} /> v{u.version} available
      </span>
    );
  }
  if (u.status === "uptodate") {
    return (
      <span className="sup-upd sup-upd--good">
        <CheckCircle2 size={14} /> Up to date
      </span>
    );
  }
  if (u.status === "error" && u.error) {
    return (
      <span className="sup-upd sup-upd--bad" title={u.error}>
        <TriangleAlert size={14} /> Check failed
      </span>
    );
  }
  return null;
}
