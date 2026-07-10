import { describe, expect, it } from "vitest";
import { mockApi } from "./mock";

describe("mockApi", () => {
  it("returns numeric server metrics", async () => {
    const m = await mockApi.getMetrics();
    expect(typeof m.serverfps).toBe("number");
    expect(typeof m.uptime).toBe("number");
    expect(m.maxplayernum).toBeGreaterThan(0);
  });

  it("returns a non-empty player roster", async () => {
    const players = await mockApi.getPlayers();
    expect(players.length).toBeGreaterThan(0);
    expect(players[0].name).toBeTruthy();
  });

  it("returns settings with the documented keys", async () => {
    const s = await mockApi.getSettings();
    expect(s.ServerName).toBeTruthy();
    expect(Object.keys(s).length).toBeGreaterThanOrEqual(60);
  });

  it("returns a game-data snapshot with players and wild pals", async () => {
    const g = await mockApi.getGameData();
    const types = new Set(g.ActorData.map((a) => a.UnitType));
    expect(types.has("Player")).toBe(true);
    expect(types.has("WildPal")).toBe(true);
    expect(g.ActorData.length).toBeGreaterThan(50);
  });

  it("resolves action calls as ok", async () => {
    expect((await mockApi.announce("hi")).ok).toBe(true);
    expect((await mockApi.saveWorld()).ok).toBe(true);
    expect((await mockApi.shutdown(30, "bye")).ok).toBe(true);
  });
});
