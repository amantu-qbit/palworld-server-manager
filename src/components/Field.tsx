import type { InputHTMLAttributes, ReactNode } from "react";

interface FieldProps {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
}

/** Labelled form-field wrapper. */
export function Field({ label, hint, children }: FieldProps) {
  return (
    <label className="field">
      <span className="field__label">{label}</span>
      {children}
      {hint && <span className="field__hint">{hint}</span>}
    </label>
  );
}

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  mono?: boolean;
}

export function Input({ mono, className = "", ...rest }: InputProps) {
  return <input className={`input${mono ? " input--mono" : ""} ${className}`} {...rest} />;
}
