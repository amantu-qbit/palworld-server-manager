import { useQuery } from "@tanstack/react-query";
import { bridgeApi } from "../api/bridge";
import { useConnection } from "../store/connection";

/**
 * Tier-2 feature-detection. When the connection carries a bridge port + token,
 * probes `GET /v1/health`; the "Server+" nav group and Characters screen appear
 * only when that probe succeeds. No bridge configured ⇒ Tier 1, everything off.
 */
export function useBridge() {
  const { connection } = useConnection();
  const configured = !!(connection?.bridgePort && connection?.bridgeToken);
  const health = useQuery({
    queryKey: ["bridge", "health"],
    queryFn: () => bridgeApi.health(),
    enabled: configured,
    retry: 0,
    staleTime: 15_000,
    refetchInterval: configured ? 30_000 : false,
  });
  return {
    configured,
    available: configured && health.isSuccess,
    checking: configured && health.isLoading,
    health: health.data,
    error: health.isError ? health.error : null,
  };
}

export function useBridgePlayers() {
  return useQuery({
    queryKey: ["bridge", "players"],
    queryFn: () => bridgeApi.players(),
    staleTime: 5_000,
  });
}

export function useBridgePlayerDetail(uid: string | null) {
  return useQuery({
    queryKey: ["bridge", "player", uid],
    queryFn: () => bridgeApi.playerDetail(uid as string),
    enabled: !!uid,
  });
}

export function useBridgeGuilds() {
  return useQuery({
    queryKey: ["bridge", "guilds"],
    queryFn: () => bridgeApi.guilds(),
    staleTime: 30_000,
  });
}

/** id → display-name catalog (items, active_skills, …). Effectively static. */
export function useBridgeReference(catalog: string) {
  return useQuery({
    queryKey: ["bridge", "reference", catalog],
    queryFn: () => bridgeApi.reference(catalog),
    staleTime: Infinity,
  });
}

/** Raw Save debug: the `.sav` files the bridge can see under its save dir. */
export function useBridgeSavFiles() {
  return useQuery({
    queryKey: ["bridge", "savfiles"],
    queryFn: () => bridgeApi.savFiles(),
    staleTime: 30_000,
  });
}

/** Raw Save debug: one subtree of a `.sav` (lazily fetched as nodes expand). */
export function useBridgeSavTree(file: string | null, path: string, page?: number, depth?: number) {
  return useQuery({
    queryKey: ["bridge", "savtree", file, path, page, depth],
    queryFn: () => bridgeApi.savTree(file as string, path, page, depth),
    enabled: !!file,
    staleTime: 10_000,
    retry: 0,
  });
}

/** Live process-supervisor status, polled while the Server Control screen is open. */
export function useServerStatus(enabled: boolean) {
  return useQuery({
    queryKey: ["bridge", "server-status"],
    queryFn: () => bridgeApi.serverStatus(),
    enabled,
    retry: 0,
    refetchInterval: enabled ? 4000 : false,
  });
}
