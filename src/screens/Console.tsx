import { SquareTerminal } from "lucide-react";
import { TopBar } from "../components/TopBar";
import { EmptyState } from "../components/EmptyState";

export function Console() {
  return (
    <>
      <TopBar breadcrumb="Control" title="Console" showLive={false} />
      <div className="page">
        <EmptyState icon={SquareTerminal} title="Console" detail="Command console — building next." />
      </div>
    </>
  );
}
