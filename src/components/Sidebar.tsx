import type { LucideIcon } from "lucide-react";
import { LogOut, PanelLeft, PanelLeftClose } from "lucide-react";

export interface NavItem {
  id: string;
  label: string;
  icon: LucideIcon;
  group: string;
}

interface Props {
  items: NavItem[];
  active: string;
  onSelect: (id: string) => void;
  host: string;
  connected: boolean;
  onDisconnect: () => void;
  collapsed: boolean;
  onToggleCollapse: () => void;
}

export function Sidebar({ items, active, onSelect, host, connected, onDisconnect, collapsed, onToggleCollapse }: Props) {
  const groups = items.reduce<Record<string, NavItem[]>>((acc, it) => {
    (acc[it.group] ??= []).push(it);
    return acc;
  }, {});

  return (
    <aside className="sidebar">
      <div className="sidebar__brand">
        <div className="brandmark" aria-hidden />
        <div className="sidebar__brandtext">
          <b>Palworld</b>
          <span>Server Manager</span>
        </div>
      </div>

      <nav className="sidebar__nav">
        {Object.entries(groups).map(([group, list]) => (
          <div key={group} className="sidebar__group">
            <div className="eyebrow sidebar__grouplabel">{group}</div>
            {list.map((it) => {
              const Icon = it.icon;
              return (
                <button
                  key={it.id}
                  className={`nav${active === it.id ? " is-active" : ""}`}
                  onClick={() => onSelect(it.id)}
                  aria-current={active === it.id ? "page" : undefined}
                  title={it.label}
                >
                  <Icon />
                  <span className="nav__label">{it.label}</span>
                </button>
              );
            })}
          </div>
        ))}
      </nav>

      <div className="sidebar__foot">
        <button
          className="nav sidebar__collapse"
          onClick={onToggleCollapse}
          title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          {collapsed ? <PanelLeft /> : <PanelLeftClose />}
          <span className="nav__label">Collapse</span>
        </button>
        <div className="sidebar__conn">
          <span className="chip__dot" style={{ color: connected ? "var(--good)" : "var(--faint)" }} />
          <div className="sidebar__connmeta">
            <b className="mono">{host}</b>
            <small>{connected ? "Connected" : "Offline"}</small>
          </div>
          <button className="icobtn" onClick={onDisconnect} aria-label="Disconnect" title="Disconnect">
            <LogOut size={15} />
          </button>
        </div>
      </div>
    </aside>
  );
}
