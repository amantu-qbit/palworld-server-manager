import { useMemo, useState } from "react";
import { Search, X } from "lucide-react";
import { TechIcon } from "./TechIcon";
import {
  ANCIENT_POINT_ICON,
  TECH_POINT_ICON,
  TECH_TOTAL,
  techCell,
  techTree,
  type TechMeta,
} from "../lib/techDex";

type Filter = "all" | "unlocked" | "locked";
const KIND_LABEL: Record<string, string> = { structure: "Structures", item: "Items", other: "" };

/**
 * The in-game technology tree for one player: every technology in the game,
 * grouped by unlock level, laid out exactly like Palworld's Technology screen —
 * an eight-wide regular grid on the left and a separate "Ancient Technology"
 * column on the right. Unlocked techs show in full colour; locked ones are
 * dimmed and show their point cost. A filter bar hides/shows locked or unlocked
 * techs and searches by name.
 */
export function TechTree({
  unlocked,
  techPoints,
  ancientPoints,
}: {
  unlocked: string[];
  techPoints: number;
  ancientPoints: number;
}) {
  const tree = techTree();
  const unlockedSet = useMemo(() => new Set(unlocked.map((c) => c.toLowerCase())), [unlocked]);
  // Level-1 techs are always available in game, even when absent from the save.
  const isOn = (m: TechMeta) => m.level <= 1 || unlockedSet.has(m.code.toLowerCase());

  const [onCount, ancOnCount] = useMemo(() => {
    let on = 0;
    let anc = 0;
    for (const row of tree) {
      for (const m of row.regular) if (isOn(m)) on++;
      if (row.ancient && isOn(row.ancient)) {
        on++;
        anc++;
      }
    }
    return [on, anc];
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tree, unlockedSet]);

  const [filter, setFilter] = useState<Filter>("all");
  const [search, setSearch] = useState("");
  const q = search.trim().toLowerCase();

  const match = (m: TechMeta) => {
    if (filter === "unlocked" && !isOn(m)) return false;
    if (filter === "locked" && isOn(m)) return false;
    if (q && !m.name.toLowerCase().includes(q)) return false;
    return true;
  };

  const rows = useMemo(() => {
    return tree
      .map((row) => ({
        level: row.level,
        regular: row.regular.filter(match),
        ancient: row.ancient && match(row.ancient) ? row.ancient : null,
        hasAncientSlot: !!row.ancient,
      }))
      .filter((r) => r.regular.length > 0 || r.ancient);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tree, filter, q, unlockedSet]);

  const [tip, setTip] = useState<{ m: TechMeta; on: boolean; rect: DOMRect } | null>(null);
  const hover = (m: TechMeta, el: HTMLElement) => setTip({ m, on: isOn(m), rect: el.getBoundingClientRect() });
  const clear = () => setTip(null);

  return (
    <div className="tt">
      <div className="tt-head">
        <div className="tt-pt tt-pt--tech">
          <img src={TECH_POINT_ICON} alt="" width={26} height={26} />
          <div className="tt-pt__txt">
            <b>{techPoints.toLocaleString()}</b>
            <span>Technology Points</span>
          </div>
        </div>
        <div className="tt-pt tt-pt--anc">
          <img src={ANCIENT_POINT_ICON} alt="" width={26} height={26} />
          <div className="tt-pt__txt">
            <b>{ancientPoints.toLocaleString()}</b>
            <span>Ancient Points</span>
          </div>
        </div>
        <div className="tt-prog">
          <div className="tt-prog__n">
            <b>{onCount}</b>
            <span>/ {TECH_TOTAL} unlocked</span>
          </div>
          <div className="tt-prog__bar">
            <i style={{ width: `${(onCount / TECH_TOTAL) * 100}%` }} />
          </div>
          <div className="tt-prog__anc">{ancOnCount} Ancient</div>
        </div>
      </div>

      <div className="tt-filters">
        <div className="tt-seg" role="tablist">
          <button className={filter === "all" ? "is-on" : ""} onClick={() => setFilter("all")}>
            All <span>{TECH_TOTAL}</span>
          </button>
          <button className={filter === "unlocked" ? "is-on" : ""} onClick={() => setFilter("unlocked")}>
            Unlocked <span>{onCount}</span>
          </button>
          <button className={filter === "locked" ? "is-on" : ""} onClick={() => setFilter("locked")}>
            Locked <span>{TECH_TOTAL - onCount}</span>
          </button>
        </div>
        <div className="tt-search">
          <Search size={13} />
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search technologies…"
          />
          {search && (
            <button className="tt-search__clear" onClick={() => setSearch("")} aria-label="Clear search">
              <X size={12} />
            </button>
          )}
        </div>
      </div>

      {rows.length === 0 ? (
        <p className="ch-empty">No technologies match this filter.</p>
      ) : (
        <div className="tt-board">
          <div className="tt-boardinner">
            <div className="tt-colhead">
              <span className="tt-colhead__tech">Technology</span>
              <span className="tt-colhead__spacer" />
              <span className="tt-colhead__anc">Ancient Technology</span>
            </div>
            {rows.map((row) => {
              const slots: (TechMeta | null)[] = [...row.regular];
              while (slots.length < 8) slots.push(null);
              return (
                <div key={row.level} className="tt-row">
                <div className="tt-lvl">
                  <span className="tt-lvl__n">
                    <i>{row.level}</i>
                  </span>
                </div>
                {slots.map((m, i) =>
                  m ? (
                    <Tile key={m.code} m={m} on={isOn(m)} onHover={hover} onLeave={clear} />
                  ) : (
                    <span key={`e${i}`} className="tt-slot" />
                  ),
                )}
                <span className="tt-div" />
                <div className="tt-anc">
                  {row.ancient ? (
                    <Tile m={row.ancient} on={isOn(row.ancient)} onHover={hover} onLeave={clear} />
                  ) : (
                    <span className="tt-anc__empty" />
                  )}
                </div>
              </div>
              );
              })}
            </div>
          </div>
      )}

      {tip && <Tooltip tip={tip} />}
    </div>
  );
}

function Tile({
  m,
  on,
  onHover,
  onLeave,
}: {
  m: TechMeta;
  on: boolean;
  onHover: (m: TechMeta, el: HTMLElement) => void;
  onLeave: () => void;
}) {
  const cell = techCell(m.code);
  const kind = KIND_LABEL[m.kind];
  return (
    <button
      type="button"
      className={`tt-tile tt-tile--${m.boss ? "anc" : "tech"} ${on ? "is-on" : "is-off"}`}
      onMouseEnter={(e) => onHover(m, e.currentTarget)}
      onMouseLeave={onLeave}
      onFocus={(e) => onHover(m, e.currentTarget)}
      onBlur={onLeave}
    >
      {kind && <span className="tt-tile__kind">{kind}</span>}
      <TechIcon cell={cell} />
      {!on && m.cost > 0 && <span className="tt-tile__cost">{m.cost}</span>}
      <span className="tt-tile__name">{m.name}</span>
    </button>
  );
}

function Tooltip({ tip }: { tip: { m: TechMeta; on: boolean; rect: DOMRect } }) {
  const { m, on, rect } = tip;
  const W = 260;
  const left = Math.max(8, Math.min(rect.left + rect.width / 2 - W / 2, window.innerWidth - W - 8));
  const below = rect.top < 190;
  const style: React.CSSProperties = below
    ? { left, top: rect.bottom + 8, width: W }
    : { left, bottom: window.innerHeight - rect.top + 8, width: W };
  return (
    <div className={`tt-pop ${m.boss ? "tt-pop--anc" : ""}`} style={style} role="tooltip">
      <div className="tt-pop__top">
        <span className="tt-pop__name">{m.name}</span>
        <span className="tt-pop__cost">
          <img src={m.boss ? ANCIENT_POINT_ICON : TECH_POINT_ICON} alt="" width={14} height={14} />
          {m.cost}
        </span>
      </div>
      <div className="tt-pop__meta">
        {m.boss ? "Ancient Technology" : KIND_LABEL[m.kind] || "Technology"}
        <span className={`tt-pop__state ${on ? "is-on" : ""}`}>
          {on ? "Unlocked" : `Requires Lv ${m.level}`}
        </span>
      </div>
      {m.desc && <p className="tt-pop__desc">{m.desc}</p>}
    </div>
  );
}
