import { AnimatePresence, motion } from "framer-motion";
import { AlertTriangle } from "lucide-react";
import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { Button } from "./Button";

export interface ConfirmSpec {
  title: string;
  body: ReactNode;
  confirmText?: string;
  danger?: boolean;
  /** If set, the user must type this string to enable the confirm button. */
  requireText?: string;
  onConfirm: () => void | Promise<void>;
}

interface Props {
  spec: ConfirmSpec | null;
  onClose: () => void;
}

/** Modal confirmation, with optional typed-phrase gate for destructive actions. */
export function ConfirmDialog({ spec, onClose }: Props) {
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setTyped("");
    setBusy(false);
  }, [spec]);

  const canConfirm = !spec?.requireText || typed.trim() === spec.requireText;

  const confirm = async () => {
    if (!spec || !canConfirm) return;
    setBusy(true);
    try {
      await spec.onConfirm();
      onClose();
    } finally {
      setBusy(false);
    }
  };

  return (
    <AnimatePresence>
      {spec && (
        <motion.div
          className="scrim modal-wrap"
          style={{ zIndex: "var(--z-modal)" as unknown as number }}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={onClose}
        >
          <motion.div
            className="modal card"
            initial={{ opacity: 0, scale: 0.94, y: 16 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.96, y: 8 }}
            transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
            role="alertdialog"
            aria-label={spec.title}
            onClick={(e) => e.stopPropagation()}
          >
            <div className={`modal__icon${spec.danger ? " modal__icon--danger" : ""}`}>
              <AlertTriangle size={20} />
            </div>
            <h2 className="modal__title">{spec.title}</h2>
            <div className="modal__body">{spec.body}</div>
            {spec.requireText && (
              <input
                className="input input--mono modal__gate"
                placeholder={`Type "${spec.requireText}" to confirm`}
                value={typed}
                onChange={(e) => setTyped(e.target.value)}
                autoFocus
              />
            )}
            <div className="modal__actions">
              <Button variant="ghost" onClick={onClose}>
                Cancel
              </Button>
              <Button
                variant={spec.danger ? "danger" : "primary"}
                onClick={confirm}
                disabled={!canConfirm}
                loading={busy}
              >
                {spec.confirmText ?? "Confirm"}
              </Button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
