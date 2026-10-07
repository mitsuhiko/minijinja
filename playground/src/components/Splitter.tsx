import { useRef } from "react";

interface Props {
  direction: "horizontal" | "vertical";
  /** Called with the pointer position relative to the parent element (0..1). */
  onResize: (fraction: number) => void;
}

/** A draggable divider between two panes. */
export function Splitter({ direction, onResize }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  const update = (event: React.PointerEvent) => {
    const parent = ref.current?.parentElement;
    if (!parent) {
      return;
    }
    const rect = parent.getBoundingClientRect();
    const fraction =
      direction === "horizontal"
        ? (event.clientX - rect.left) / rect.width
        : (event.clientY - rect.top) / rect.height;
    onResize(Math.min(0.85, Math.max(0.15, fraction)));
  };

  return (
    <div
      ref={ref}
      className={`splitter splitter-${direction}`}
      role="separator"
      aria-orientation={direction === "horizontal" ? "vertical" : "horizontal"}
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        document.body.classList.add(`resizing-${direction}`);
      }}
      onPointerMove={(event) => {
        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
          update(event);
        }
      }}
      onPointerUp={(event) => {
        event.currentTarget.releasePointerCapture(event.pointerId);
        document.body.classList.remove(`resizing-${direction}`);
      }}
    />
  );
}
