// SPDX-License-Identifier: AGPL-3.0-or-later

export interface ToggleSwitchProps {
  id: string;
  label: string;
  hint: string;
  checked: boolean;
  disabled?: boolean;
  onToggle: () => void;
}

export function ToggleSwitch({
  id,
  label,
  hint,
  checked,
  disabled = false,
  onToggle,
}: Readonly<ToggleSwitchProps>) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      className="switch-row"
      onClick={onToggle}
    >
      <span className="switch-copy">
        <span className="switch-label">{label}</span>
        <span className="switch-hint">{hint}</span>
      </span>
      <span className="switch-track" aria-hidden="true">
        <span className="switch-thumb" />
      </span>
    </button>
  );
}
