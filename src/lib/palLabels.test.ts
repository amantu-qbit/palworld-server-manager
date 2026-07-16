import { describe, expect, it } from "vitest";
import { statusLabel } from "./palLabels";

describe("statusLabel", () => {
  it("maps enum forms to English", () => {
    expect(statusLabel("MaxHP")).toBe("Health");
    expect(statusLabel("EPalStatusName::MaxSP")).toBe("Stamina");
    expect(statusLabel("WorkSpeed")).toBe("Work Speed");
  });

  it("maps known Japanese 1.0 status names to English", () => {
    expect(statusLabel("最大HP")).toBe("Health");
    expect(statusLabel("スタミナ消費軽減")).toBe("Stamina Cost");
    expect(statusLabel("登り速度")).toBe("Climb Speed");
    expect(statusLabel("状態異常耐性")).toBe("Ailment Resist");
    expect(statusLabel("経験値ボーナス")).toBe("EXP Bonus");
    expect(statusLabel("食料腐敗低減")).toBe("Food Preservation");
  });

  it("never leaks unmapped CJK text into the UI", () => {
    // A status name we don't have an explicit English label for still must not
    // render Japanese/Chinese/Korean characters.
    for (const s of ["紅イッシブ率", "謎のステータス", "미지의스탯", "神秘属性"]) {
      const out = statusLabel(s);
      expect(out).not.toMatch(/[　-〿぀-ヿ㐀-䶿一-鿿가-힯]/);
    }
  });

  it("leaves plain ASCII labels readable", () => {
    expect(statusLabel("Movement Speed")).toBe("Movement Speed");
  });
});
