import { useEffect, useRef, useState } from "react";

export interface SelectOption {
  value: string;
  label: string;
}

interface Props {
  value: string;
  options: SelectOption[];
  onChange(v: string): void;
  placeholder?: string;
  className?: string;
}

/** Custom select with consistent option-row sizing across OSes
 *  (native <select> dropdowns ignore our font-size on macOS). */
export function Select({ value, options, onChange, placeholder, className = "" }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const selected = options.find((o) => o.value === value);

  useEffect(() => {
    function onDocClick(e: MouseEvent) {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    if (open) {
      window.addEventListener("mousedown", onDocClick);
      window.addEventListener("keydown", onKey);
    }
    return () => {
      window.removeEventListener("mousedown", onDocClick);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={ref} className={"relative " + className}>
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="w-full bg-slate-800 hover:bg-slate-700 rounded px-3 py-2.5 text-base text-left flex items-center justify-between gap-2"
      >
        <span className={selected ? "" : "text-slate-500"}>
          {selected?.label ?? placeholder ?? "선택"}
        </span>
        <span className="text-slate-400 text-xs">▾</span>
      </button>
      {open && (
        <ul className="absolute z-50 left-0 right-0 top-full mt-1 bg-slate-900 border border-slate-700 rounded shadow-xl max-h-72 overflow-y-auto py-1">
          {options.map((o) => {
            const isSelected = o.value === value;
            return (
              <li
                key={o.value}
                onClick={() => { onChange(o.value); setOpen(false); }}
                className={
                  "px-3 py-2.5 text-base cursor-pointer hover:bg-slate-700 " +
                  (isSelected ? "bg-slate-800 text-emerald-400" : "text-slate-100")
                }
              >
                {o.label}
              </li>
            );
          })}
          {options.length === 0 && (
            <li className="px-3 py-2.5 text-base text-slate-500">옵션 없음</li>
          )}
        </ul>
      )}
    </div>
  );
}
