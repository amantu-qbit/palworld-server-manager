import { useState } from "react";
import type { ComponentType } from "react";
import {
  LayoutDashboard,
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

const NAV: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard, group: "Overview" },
  { id: "players", label: "Players", icon: Users, group: "Overview" },
  { id: "map", label: "World Map", icon: Radar, group: "Overview" },
  { id: "console", label: "Console", icon: SquareTerminal, group: "Control" },
  { id: "settings", label: "Settings", icon: SlidersHorizontal, group: "Control" },
  { id: "bans", label: "Ban Manager", icon: ShieldBan, group: "Control" },
];

const SCREENS: Record<string, ComponentType> = {
  dashboard: Dashboard,
  players: Players,
  map: WorldMap,
  console: Console,
  settings: Settings,
  bans: BanManager,
};

export function App() {
  const { connected, connection, disconnect } = useConnection();
  const [active, setActive] = useState("dashboard");

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
          <Screen />
        </div>
      </div>
    </NavContext.Provider>
  );
}
