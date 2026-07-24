import { useEffect, useId, useRef, useState } from "react";
import { ChevronDownIcon } from "./icons";

export interface FormSelectOption {
  value: string;
  label: string;
}

export function FormSelect({
  ariaLabel,
  value,
  options,
  onChange,
  autoFocus = false,
  placeholder,
}: {
  ariaLabel: string;
  value: string;
  options: readonly FormSelectOption[];
  onChange: (value: string) => void;
  autoFocus?: boolean;
  placeholder?: string;
}) {
  const [open, setOpen] = useState(false);
  const selectedIndex = Math.max(0, options.findIndex((option) => option.value === value));
  const [activeIndex, setActiveIndex] = useState(selectedIndex);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listboxId = useId();
  const selected = options.find((option) => option.value === value);

  const openMenu = () => {
    setActiveIndex(selectedIndex);
    setOpen(true);
  };

  const choose = (index: number) => {
    const option = options[index];
    if (!option) return;
    onChange(option.value);
    setActiveIndex(index);
    setOpen(false);
    triggerRef.current?.focus();
  };

  useEffect(() => {
    if (!open) return undefined;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", closeOnOutsidePointer);
    return () => document.removeEventListener("pointerdown", closeOnOutsidePointer);
  }, [open]);

  useEffect(() => {
    if (open) setActiveIndex(selectedIndex);
  }, [open, selectedIndex]);

  const move = (delta: number) => {
    if (options.length === 0) return;
    setActiveIndex((current) => (current + delta + options.length) % options.length);
  };

  return (
    <div className="mux-form-select" ref={rootRef}>
      <button
        ref={triggerRef}
        type="button"
        role="combobox"
        aria-label={ariaLabel}
        aria-controls={listboxId}
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-activedescendant={open ? `${listboxId}-option-${activeIndex}` : undefined}
        autoFocus={autoFocus}
        className="mux-model-field mux-form-select-trigger"
        data-open={open ? "true" : undefined}
        onClick={() => (open ? setOpen(false) : openMenu())}
        onKeyDown={(event) => {
          switch (event.key) {
            case "ArrowDown":
              event.preventDefault();
              if (open) move(1);
              else openMenu();
              break;
            case "ArrowUp":
              event.preventDefault();
              if (open) move(-1);
              else {
                setActiveIndex(options.length - 1);
                setOpen(true);
              }
              break;
            case "Home":
              if (open) {
                event.preventDefault();
                setActiveIndex(0);
              }
              break;
            case "End":
              if (open) {
                event.preventDefault();
                setActiveIndex(Math.max(0, options.length - 1));
              }
              break;
            case "Enter":
            case " ":
              event.preventDefault();
              if (open) choose(activeIndex);
              else openMenu();
              break;
            case "Escape":
              if (open) {
                event.preventDefault();
                setOpen(false);
              }
              break;
            case "Tab":
              setOpen(false);
              break;
          }
        }}
      >
        <span className="mux-form-select-value">{selected?.label ?? placeholder ?? ""}</span>
        <ChevronDownIcon className="mux-form-select-chevron" />
      </button>

      {open && (
        <div id={listboxId} className="mux-form-select-menu" role="listbox" aria-label={ariaLabel}>
          {options.map((option, index) => (
            <button
              id={`${listboxId}-option-${index}`}
              key={option.value}
              type="button"
              role="option"
              aria-selected={option.value === value}
              className="mux-form-select-option"
              data-active={index === activeIndex ? "true" : undefined}
              data-selected={option.value === value ? "true" : undefined}
              onPointerMove={() => setActiveIndex(index)}
              onClick={() => choose(index)}
            >
              <span>{option.label}</span>
              <span className="mux-form-select-option-mark" aria-hidden="true">
                {option.value === value ? "✓" : ""}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
