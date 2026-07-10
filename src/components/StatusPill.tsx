type Tone = "good" | "warn" | "bad" | "accent";

interface Props {
  tone: Tone;
  label: string;
  pulse?: boolean;
}

/** Small status chip with a glowing dot. */
export function StatusPill({ tone, label, pulse }: Props) {
  return (
    <span className={`chip chip--${tone}`}>
      <span className={pulse ? "livedot" : "chip__dot"} />
      {label}
    </span>
  );
}
