import { useEffect, useRef } from "react";

type DropdownOption = {
  value: string;
  label: string;
};

type DropdownSelectProps = {
  value: string;
  options: DropdownOption[];
  open: boolean;
  disabled?: boolean;
  emptyLabel: string;
  menuLabel: string;
  onOpenChange: (open: boolean) => void;
  onChange: (value: string) => void;
};

export function DropdownSelect({
  value,
  options,
  open,
  disabled = false,
  emptyLabel,
  menuLabel,
  onOpenChange,
  onChange,
}: DropdownSelectProps) {
  const dropdownRef = useRef<HTMLDivElement>(null);

  const selectedOption = options.find((entry) => entry.value === value) ?? options[0] ?? null;
  const selectedLabel = selectedOption?.label ?? emptyLabel;

  useEffect(() => {
    if (!open) {
      return undefined;
    }

    const handlePointerDown = (event: MouseEvent) => {
      if (!dropdownRef.current?.contains(event.target as Node)) {
        onOpenChange(false);
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onOpenChange(false);
      }
    };

    window.addEventListener("mousedown", handlePointerDown);
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("mousedown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [onOpenChange, open]);

  useEffect(() => {
    if (open && (disabled || options.length === 0)) {
      onOpenChange(false);
    }
  }, [disabled, onOpenChange, open, options.length]);

  return (
    <div className="dropdown-select" ref={dropdownRef}>
      <button
        type="button"
        className="dropdown-trigger"
        disabled={disabled}
        aria-expanded={open}
        onClick={() => onOpenChange(!open)}
      >
        <span>{selectedLabel}</span>
        <span className={`dropdown-arrow ${open ? "is-open" : ""}`} aria-hidden="true">
          v
        </span>
      </button>

      {open ? (
        <div className="dropdown-menu" role="listbox" aria-label={menuLabel}>
          {options.map((entry) => {
            const isSelected = entry.value === selectedOption?.value;

            return (
              <button
                type="button"
                key={entry.value}
                className={`dropdown-option ${isSelected ? "is-selected" : ""}`}
                onClick={() => {
                  onChange(entry.value);
                  onOpenChange(false);
                }}
              >
                {entry.label}
              </button>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}