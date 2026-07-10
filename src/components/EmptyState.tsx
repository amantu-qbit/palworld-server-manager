import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

interface Props {
  icon: LucideIcon;
  title: string;
  detail?: string;
  children?: ReactNode;
  tone?: "default" | "error";
}

/** Centered empty / error placeholder. */
export function EmptyState({ icon: Icon, title, detail, children, tone = "default" }: Props) {
  return (
    <div className="empty">
      <div className={`empty__icon${tone === "error" ? " empty__icon--error" : ""}`}>
        <Icon size={24} />
      </div>
      <h3>{title}</h3>
      {detail && <p>{detail}</p>}
      {children}
    </div>
  );
}
