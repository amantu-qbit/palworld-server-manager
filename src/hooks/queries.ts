import { QueryClient, useQuery } from "@tanstack/react-query";
import { api } from "../api";
import { usePrefs } from "../store/prefs";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
      staleTime: 1000,
    },
  },
});

/** Shared interval derived from prefs (0 = paused → no auto-refetch). */
function useInterval(): number | false {
  const { pollInterval, paused } = usePrefs();
  return paused ? false : pollInterval;
}

export function useInfo() {
  return useQuery({ queryKey: ["info"], queryFn: () => api.getInfo(), staleTime: 30_000 });
}

export function useMetrics() {
  const refetchInterval = useInterval();
  return useQuery({ queryKey: ["metrics"], queryFn: () => api.getMetrics(), refetchInterval });
}

export function usePlayers() {
  const refetchInterval = useInterval();
  return useQuery({ queryKey: ["players"], queryFn: () => api.getPlayers(), refetchInterval });
}

export function useGameData() {
  const refetchInterval = useInterval();
  return useQuery({ queryKey: ["gameData"], queryFn: () => api.getGameData(), refetchInterval });
}

export function useSettings() {
  return useQuery({ queryKey: ["settings"], queryFn: () => api.getSettings(), staleTime: 60_000 });
}
