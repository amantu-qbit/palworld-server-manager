import { useMemo, useState } from "react";
import { TechIcon } from "./TechIcon";
import {
  ANCIENT_POINT_ICON,
  TECH_POINT_ICON,
  TECH_TOTAL,
  techCell,
  techTree,
  type TechMeta,
} from "../lib/techDex";

/**
 * The in-game technology tree for one player: every technology in the game,
 * grouped by unlock level, with the player's unlocked techs lit and the rest
 * dimmed — the layout Palworld and palworld-save-pal both use (level rail on
 * the left, up to eight regular tiles, then the level's single Ancient tech).
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
  const unlockedSet = useMemo(
    () => new Set(unlocked.map((c) => c.toLowerCase())),
    [unlocked],
  );
  // Level-1 techs are always available in game, even when absent from the save.
  const isOn = (m: TechMeta) => m.level <= 1 || unlockedSet.has(m.code.toLowerCase());

  const [count, ancientCount] = useMemo(() => {
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
            <b>{count}</b>
            <span>/ {TECH_TOTAL} unlocked</span>
          </div>
          <div className="tt-prog__bar">
            <i style={{ width: `${(count / TECH_TOTAL) * 100}%` }} />
          </div>
          <div className="tt-prog__anc">{ancientCount} Ancient</div>
        </div>
      </div>

      <div className="tt-board">
        {tree.map((row) => (
          <div key={row.level} className="tt-row">
            <div className="tt-lvl">
              <span className="tt-lvl__n">{row.level}</span>
              <span className="tt-lvl__rail" />
            </div>
            <div className="tt-tiles">
              {row.regular.map((m) => (
                <Tile key={m.code} m={m} on={isOn(m)} onHover={hover} onLeave={clear} />
              ))}
            </div>
            <span className="tt-div" />
            <div className="tt-anc">
              {row.ancient ? (
                <Tile m={row.ancient} on={isOn(row.ancient)} onHover={hover} onLeave={clear} />
              ) : (
                <span className="tt-anc__empty" />
              )}
            </div>
          </div>
        ))}
      </div>

      {tip && <Tooltip tip={tip} />}
    </div>
  );
}

const KIND_LABEL: Record<string, string> = { structure: "Structure", item: "Item", other: "" };

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
      <TechIcon cell={cell} size={38} />
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
