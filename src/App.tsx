import { useState } from "react";
import type { ComponentType } from "react";
import {
  Boxes,
  Braces,
  Castle,
  Heart,
  LayoutDashboard,
  LineChart,
  Loader2,
  PawPrint,
  Power,
  Radar,
  ShieldBan,
  SlidersHorizontal,
  SquareTerminal,
  Users,
} from "lucide-react";
import { Sidebar } from "./components/Sidebar";
import type { NavItem } from "./components/Sidebar";
import { NavContext } from "./store/nav";
import { useConnection } from "./store/connection";
import { useBridge } from "./hooks/bridge";
import { Connect } from "./screens/Connect";
import { Dashboard } from "./screens/Dashboard";
import { Players } from "./screens/Players";
import { WorldMap } from "./screens/WorldMap";
import { Console } from "./screens/Console";
import { Trends } from "./screens/Trends";
import { Settings } from "./screens/Settings";
import { BanManager } from "./screens/BanManager";
import { Characters } from "./screens/Characters";
import { Storage } from "./screens/Storage";
import { Guilds } from "./screens/Guilds";
import { ServerControl } from "./screens/ServerControl";
import { RawSave } from "./screens/RawSave";
import { Support } from "./screens/Support";
import { UpdateBanner } from "./components/UpdateBanner";
import { MetricsRecorder } from "./components/MetricsRecorder";

const NAV: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard, group: "Overview" },
  { id: "players", label: "Players", icon: Users, group: "Overview" },
  { id: "map", label: "World Map", icon: Radar, group: "Overview" },
  { id: "trends", label: "Trends", icon: LineChart, group: "Overview" },
  { id: "console", label: "Console", icon: SquareTerminal, group: "Control" },
  { id: "settings", label: "Settings", icon: SlidersHorizontal, group: "Control" },
  { id: "bans", label: "Ban Manager", icon: ShieldBan, group: "Control" },
  { id: "support", label: "Support", icon: Heart, group: "About" },
];

const SCREENS: Record<string, ComponentType> = {
  dashboard: Dashboard,
  players: Players,
  map: WorldMap,
  trends: Trends,
  console: Console,
  settings: Settings,
  bans: BanManager,
  characters: Characters,
  storage: Storage,
  guilds: Guilds,
  servercontrol: ServerControl,
  rawsave: RawSave,
  support: Support,
};

/** Nav shown only when the Tier-2 bridge is detected. */
const BRIDGE_NAV: NavItem[] = [
  { id: "servercontrol", label: "Server Control", icon: Power, group: "Server+" },
  { id: "characters", label: "Characters", icon: PawPrint, group: "Server+" },
  { id: "storage", label: "Storage", icon: Boxes, group: "Server+" },
  { id: "guilds", label: "Guilds", icon: Castle, group: "Server+" },
  { id: "rawsave", label: "Raw Save", icon: Braces, group: "Server+" },
];

export function App() {
  const { connected, connection, disconnect, booting } = useConnection();
  const [active, setActive] = useState("dashboard");
  const [collapsed, setCollapsed] = useState(() => {
    try {
      return localStorage.getItem("psm.sidebar") === "collapsed";
    } catch {
      return false;
    }
  });
  const toggleCollapse = () =>
    setCollapsed((c) => {
      const next = !c;
      try {
        localStorage.setItem("psm.sidebar", next ? "collapsed" : "open");
      } catch {
        /* ignore */
      }
      return next;
    });

  const bridge = useBridge();

  if (booting && !connected) {
    return (
      <div className="boot">
        <div className="connect__logo" aria-hidden />
        <div className="boot__row">
          <Loader2 size={15} className="spin" />
          Connecting to your server…
        </div>
      </div>
    );
  }

  if (!connected || !connection) return <Connect />;

  // The "Server+" group appears only when the bridge is detected.
  const nav = bridge.available ? [...NAV, ...BRIDGE_NAV] : NAV;
  // If the bridge drops while a bridge-only screen is open, fall back gracefully.
  const bridgeOnly = BRIDGE_NAV.some((n) => n.id === active);
  const activeId = bridgeOnly && !bridge.available ? "dashboard" : active;
  const Screen = SCREENS[activeId] ?? Dashboard;

  return (
    <NavContext.Provider value={{ active: activeId, navigate: setActive }}>
      <div className={`app-shell${collapsed ? " app-shell--collapsed" : ""}`}>
        <Sidebar
          items={nav}
          active={activeId}
          onSelect={setActive}
          host={`${connection.host}:${connection.port}`}
          connected
          onDisconnect={disconnect}
          collapsed={collapsed}
          onToggleCollapse={toggleCollapse}
        />
        <div className="main">
          <MetricsRecorder />
          <UpdateBanner />
          <Screen />
        </div>
      </div>
    </NavContext.Provider>
  );
}
