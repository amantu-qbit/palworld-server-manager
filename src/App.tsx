import { useState } from "react";
import type { ComponentType } from "react";
import {
  Heart,
  LayoutDashboard,
  Loader2,
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
import { Connect } from "./screens/Connect";
import { Dashboard } from "./screens/Dashboard";
import { Players } from "./screens/Players";
import { WorldMap } from "./screens/WorldMap";
import { Console } from "./screens/Console";
import { Settings } from "./screens/Settings";
import { BanManager } from "./screens/BanManager";
import { Support } from "./screens/Support";
import { UpdateBanner } from "./components/UpdateBanner";

const NAV: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard, group: "Overview" },
  { id: "players", label: "Players", icon: Users, group: "Overview" },
  { id: "map", label: "World Map", icon: Radar, group: "Overview" },
  { id: "console", label: "Console", icon: SquareTerminal, group: "Control" },
  { id: "settings", label: "Settings", icon: SlidersHorizontal, group: "Control" },
  { id: "bans", label: "Ban Manager", icon: ShieldBan, group: "Control" },
  { id: "support", label: "Support", icon: Heart, group: "About" },
];

const SCREENS: Record<string, ComponentType> = {
  dashboard: Dashboard,
  players: Players,
  map: WorldMap,
  console: Console,
  settings: Settings,
  bans: BanManager,
  support: Support,
};

export function App() {
  const { connected, connection, disconnect, booting } = useConnection();
  const [active, setActive] = useState("dashboard");

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

  const Screen = SCREENS[active] ?? Dashboard;

  return (
    <NavContext.Provider value={{ active, navigate: setActive }}>
      <div className="app-shell">
        <Sidebar
          items={NAV}
          active={active}
          onSelect={setActive}
          host={`${connection.host}:${connection.port}`}
          connected
          onDisconnect={disconnect}
        />
        <div className="main">
          <UpdateBanner />
          <Screen />
        </div>
      </div>
    </NavContext.Provider>
  );
}
