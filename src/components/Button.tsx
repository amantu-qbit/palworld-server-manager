import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Loader2 } from "lucide-react";

type Variant = "default" | "primary" | "danger" | "ghost";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: "md" | "sm";
  block?: boolean;
  loading?: boolean;
  children: ReactNode;
}

const VARIANT: Record<Variant, string> = {
  default: "",
  primary: " btn--primary",
  danger: " btn--danger",
  ghost: " btn--ghost",
};

export function Button({
  variant = "default",
  size = "md",
  block,
  loading,
  children,
  className = "",
  disabled,
  ...rest
}: Props) {
  return (
    <button
      className={`btn${VARIANT[variant]}${size === "sm" ? " btn--sm" : ""}${block ? " btn--block" : ""} ${className}`}
      disabled={disabled || loading}
      {...rest}
    >
      {loading && <Loader2 size={16} className="spin" />}
      {children}
    </button>
  );
}
