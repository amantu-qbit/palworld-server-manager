import { isTauri } from "../api";

/**
 * Open a URL in the user's default browser. In the desktop app this hands off to
 * the OS via the opener plugin; in the browser dev build it opens a new tab.
 */
export async function openExternal(url: string): Promise<void> {
  if (isTauri()) {
    try {
      const opener = await import("@tauri-apps/plugin-opener");
      await opener.openUrl(url);
      return;
    } catch {
      /* fall through to a new tab */
    }
  }
  window.open(url, "_blank", "noopener,noreferrer");
}
