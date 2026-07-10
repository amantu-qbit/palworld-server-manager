import { SlidersHorizontal } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { EmptyState } from "../components/EmptyState";

export function Settings() {
  return (
    <>
      <TopBar breadcrumb="Control" title="Settings" showLive={false} />
      <div className="page">
        <EmptyState icon={SlidersHorizontal} title="Settings" detail="Settings inspector — building next." />
      </div>
    </>
  );
}
