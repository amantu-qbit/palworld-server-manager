import { useMutation, useQuery } from "@tanstack/react-query";
import { bridgeApi } from "../api/bridge";
import { queryClient } from "./queries";
import { useConnection } from "../store/connection";
import type {
  EditBaseBody,
  EditGuildBody,
  EditPalBody,
  EditPlayerBody,
  EditPlayerTechnologiesBody,
} from "../types/bridge";

/**
 * Tier-2 feature-detection. When the connection carries a bridge port + token,
 * probes `GET /v1/health`; the "Server+" nav group and Characters screen appear
 * only when that probe succeeds. No bridge configured ⇒ Tier 1, everything off.
 *
 * `writesEnabled` / `serverRunning` gate the save editors: edits need
 * `[safety] allow_writes` on the bridge AND a stopped game server.
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
    writesEnabled: health.data?.writes_enabled === true,
    serverRunning: health.data?.server_running === true,
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

/** Every labeled item container (player bags + guild chests). */
export function useBridgeContainers() {
  return useQuery({
    queryKey: ["bridge", "containers"],
    queryFn: () => bridgeApi.containers(),
    staleTime: 5_000,
  });
}

/**
 * After a save write, refetch everything read from the save. Reference
 * catalogs are static id→name maps, so they're the one `["bridge", …]`
 * family we leave alone.
 */
export function invalidateBridgeData() {
  return queryClient.invalidateQueries({
    queryKey: ["bridge"],
    predicate: (q) => q.queryKey[1] !== "reference",
  });
}

export function useResizeContainer() {
  return useMutation({
    mutationFn: ({ cid, slotNum }: { cid: string; slotNum: number }) =>
      bridgeApi.resizeContainer(cid, slotNum),
    onSuccess: () => invalidateBridgeData(),
  });
}

export function useSetContainerSlot() {
  return useMutation({
    mutationFn: (v: { cid: string; slotIndex: number; staticId: string; count: number }) =>
      bridgeApi.setContainerSlot(v.cid, v.slotIndex, v.staticId, v.count),
    onSuccess: () => invalidateBridgeData(),
  });
}

export function useEditPlayer() {
  return useMutation({
    mutationFn: ({ uid, body }: { uid: string; body: EditPlayerBody }) =>
      bridgeApi.editPlayer(uid, body),
    onSuccess: () => invalidateBridgeData(),
  });
}

export function useEditPlayerTechnologies() {
  return useMutation({
    mutationFn: ({ uid, body }: { uid: string; body: EditPlayerTechnologiesBody }) =>
      bridgeApi.editPlayerTechnologies(uid, body),
    onSuccess: () => invalidateBridgeData(),
  });
}

export function useEditPal() {
  return useMutation({
    mutationFn: ({ instanceId, body }: { instanceId: string; body: EditPalBody }) =>
      bridgeApi.editPal(instanceId, body),
    onSuccess: () => invalidateBridgeData(),
  });
}

export function useHealPal() {
  return useMutation({
    mutationFn: (instanceId: string) => bridgeApi.healPal(instanceId),
    onSuccess: () => invalidateBridgeData(),
  });
}

export function useDeletePal() {
  return useMutation({
    mutationFn: (instanceId: string) => bridgeApi.deletePal(instanceId),
    onSuccess: () => invalidateBridgeData(),
  });
}

export function useClonePal() {
  return useMutation({
    mutationFn: (instanceId: string) => bridgeApi.clonePal(instanceId),
    onSuccess: () => invalidateBridgeData(),
  });
}

export function useEditGuild() {
  return useMutation({
    mutationFn: ({ id, body }: { id: string; body: EditGuildBody }) =>
      bridgeApi.editGuild(id, body),
    onSuccess: () => invalidateBridgeData(),
  });
}

export function useEditBase() {
  return useMutation({
    mutationFn: ({ id, body }: { id: string; body: EditBaseBody }) => bridgeApi.editBase(id, body),
    onSuccess: () => invalidateBridgeData(),
  });
}
