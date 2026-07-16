import { useEffect, useId, useRef } from "react";
import { CheckIcon, ChevronDownIcon } from "./icons";
import { useDropdownPresence } from "../hooks/useDropdownPresence";

export interface SelectOption {
  value: string;
  label: string;
  meta?: string;
}

interface SelectMenuProps {
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  /** Shown when value is empty / unmatched. */
  placeholder?: string;
  /** Accessible name for the trigger. */
  "aria-label"?: string;
  disabled?: boolean;
  /** Stretch trigger to fill the parent width (agent-row controls). */
  stretch?: boolean;
  menuAlign?: "left" | "right";
}

/** App-styled select — same chrome as AgentPicker, without the native OS menu. */
export function SelectMenu({
  value,
  options,
  onChange,
  placeholder = "请选择",
  "aria-label": ariaLabel,
  disabled,
  stretch,
  menuAlign = "left",
}: SelectMenuProps) {
  const { open, mounted, phase, toggle, hide, setOpen } = useDropdownPresence();
  const anchorRef = useRef<HTMLDivElement>(null);
  const listId = useId();

  const selected = options.find((option) => option.value === value) ?? null;

  useEffect(() => {
    if (!open) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") hide();
    };
    const closeOnPointerDown = (event: PointerEvent) => {
      if (!anchorRef.current?.contains(event.target as Node)) hide();
    };
    document.addEventListener("keydown", closeOnEscape);
    document.addEventListener("pointerdown", closeOnPointerDown);
    return () => {
      document.removeEventListener("keydown", closeOnEscape);
      document.removeEventListener("pointerdown", closeOnPointerDown);
    };
  }, [hide, open]);

  return (
    <div
      className="mux-select-anchor"
      data-stretch={stretch ? "true" : undefined}
      ref={anchorRef}
    >
      <button
        type="button"
        className="mux-select-trigger"
        data-open={open ? "true" : undefined}
        data-placeholder={!selected ? "true" : undefined}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-label={ariaLabel}
        disabled={disabled}
        onClick={() => toggle()}
      >
        <span className="mux-select-trigger-label">
          {selected ? (
            <>
              <span className="mux-select-trigger-name">{selected.label}</span>
              {selected.meta && (
                <span className="mux-select-trigger-meta">{selected.meta}</span>
              )}
            </>
          ) : (
            <span className="mux-select-trigger-name">{placeholder}</span>
          )}
        </span>
        <ChevronDownIcon className="mux-select-chevron" />
      </button>

      {mounted && (
        <section
          className="mux-dropdown-panel mux-select-menu"
          data-align={menuAlign}
          data-phase={phase}
          id={listId}
          role="listbox"
          aria-label={ariaLabel}
        >
          {options.length === 0 ? (
            <div className="mux-select-empty">暂无选项</div>
          ) : (
            options.map((option) => {
              const active = option.value === value;
              return (
                <button
                  type="button"
                  role="option"
                  aria-selected={active}
                  key={option.value}
                  className="mux-select-option"
                  data-active={active ? "true" : undefined}
                  onClick={() => {
                    onChange(option.value);
                    setOpen(false);
                  }}
                >
                  <span className="min-w-0 flex-1">
                    <span className="mux-select-option-name">{option.label}</span>
                    {option.meta && (
                      <span className="mux-select-option-meta">{option.meta}</span>
                    )}
                  </span>
                  {active && <CheckIcon className="mux-select-check" />}
                </button>
              );
            })
          )}
        </section>
      )}
    </div>
  );
}
