import { Users } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { EmptyState } from "../components/EmptyState";

export function Players() {
  return (
    <>
      <TopBar breadcrumb="Overview" title="Players" showLive />
      <div className="page">
        <EmptyState icon={Users} title="Players" detail="Roster screen — building next." />
      </div>
    </>
  );
}
