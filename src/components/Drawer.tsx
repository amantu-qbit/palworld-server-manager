import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { useEffect } from "react";

interface Props {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle?: ReactNode;
  children: ReactNode;
  width?: number;
}

/** Right-anchored glass drawer for detail views. */
export function Drawer({ open, onClose, title, subtitle, children, width = 420 }: Props) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    if (open) window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  return (
    <AnimatePresence>
      {open && (
        <>
          <motion.div
            className="scrim"
            style={{ zIndex: "var(--z-drawer)" as unknown as number }}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
          />
          <motion.aside
            className="drawer glass"
            style={{ width }}
            initial={{ x: width + 40 }}
            animate={{ x: 0 }}
            exit={{ x: width + 40 }}
            transition={{ duration: 0.28, ease: [0.22, 1, 0.36, 1] }}
            role="dialog"
            aria-label={title}
          >
            <header className="drawer__head">
              <div>
                <h2>{title}</h2>
                {subtitle && <div className="drawer__sub">{subtitle}</div>}
              </div>
              <button className="icobtn" onClick={onClose} aria-label="Close">
                <X size={16} />
              </button>
            </header>
            <div className="drawer__body">{children}</div>
          </motion.aside>
        </>
      )}
    </AnimatePresence>
  );
}
