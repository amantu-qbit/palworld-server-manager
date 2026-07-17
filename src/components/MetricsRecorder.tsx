import { useEffect } from "react";
import { useConnection } from "../store/connection";
import { useMetrics } from "../hooks/queries";
import { recordMetrics, setActiveServer } from "../store/metricsHistory";

/**
 * Renders nothing — it keeps the persistent metrics history fed while
 * connected. Mounted once in the app shell (not inside the Dashboard) so trends
 * accrue on every screen, and records one sample per successful poll.
 */
export function MetricsRecorder() {
  const { connection } = useConnection();
  const metrics = useMetrics();

  useEffect(() => {
    if (connection) setActiveServer(connection.host, connection.port);
  }, [connection?.host, connection?.port]);

  const { data, dataUpdatedAt } = metrics;
  useEffect(() => {
    if (data) recordMetrics(data);
    // `dataUpdatedAt` ticks on every successful fetch even when the payload is
    // unchanged (react-query keeps `data` referentially stable via structural
    // sharing), so this records one sample per poll rather than only on change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dataUpdatedAt]);

  return null;
}
