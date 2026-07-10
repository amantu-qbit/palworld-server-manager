import { Radar } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { EmptyState } from "../components/EmptyState";

export function WorldMap() {
  return (
    <>
      <TopBar breadcrumb="Overview" title="World Map" showLive />
      <div className="page">
        <EmptyState icon={Radar} title="World Map" detail="Actor radar — building next." />
      </div>
    </>
  );
}
