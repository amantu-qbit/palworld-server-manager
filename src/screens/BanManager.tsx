import { ShieldBan } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { EmptyState } from "../components/EmptyState";

export function BanManager() {
  return (
    <>
      <TopBar breadcrumb="Control" title="Ban Manager" showLive={false} />
      <div className="page">
        <EmptyState icon={ShieldBan} title="Ban Manager" detail="Ban tools — building next." />
      </div>
    </>
  );
}
