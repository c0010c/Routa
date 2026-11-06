import { Box, type BoxProps } from "@mui/material";
import { useLayoutEffect, useRef } from "react";

/** Cards wrap at a preferred width; stretch optionally fills every available column. */
export function FixedCardGrid({
  width = 360,
  stretch = false,
  animate = false,
  sx,
  ...props
}: BoxProps & { width?: number; stretch?: boolean; animate?: boolean }) {
  const grid = useRef<HTMLDivElement>(null);
  const previous = useRef({ width: 0, positions: new Map<string, { x: number; y: number }>() });
  useLayoutEffect(() => {
    const root = grid.current;
    if (!animate || !root) return;
    const bounds = root.getBoundingClientRect();
    const positions = new Map<string, { x: number; y: number }>();
    root.querySelectorAll<HTMLElement>(":scope > [data-grid-key]").forEach((card) => {
      const rect = card.getBoundingClientRect(), key = card.dataset.gridKey!;
      const next = { x: rect.left - bounds.left, y: rect.top - bounds.top };
      const old = previous.current.positions.get(key);
      positions.set(key, next);
      if (old && previous.current.width === bounds.width && (old.x !== next.x || old.y !== next.y) && !window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
        card.animate?.([{ transform: `translate(${old.x - next.x}px,${old.y - next.y}px)` }, { transform: "translate(0,0)" }], { duration: 180, easing: "cubic-bezier(.2,.8,.2,1)" });
      }
    });
    previous.current = { width: bounds.width, positions };
  });
  return (
    <Box
      {...props}
      ref={grid}
      sx={[
        {
          display: "grid",
          gridTemplateColumns: stretch
            ? `repeat(auto-fill, minmax(min(${width}px, 100%), 1fr))`
            : `repeat(auto-fill, min(${width}px, 100%))`,
          gap: 2,
          alignContent: "start",
        },
        ...(Array.isArray(sx) ? sx : sx ? [sx] : []),
      ]}
    />
  );
}
