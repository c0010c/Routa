import { useId } from "react";
import { BRAND_COLORS as C } from "../brand";

// Routa 应用图标（与 designs/routa-icon-final 的 master.svg / small.svg 一致）。
// 32 px 及以下使用小尺寸版：圆角更小、轨道加粗、节点放大，远处节点也为纯白。
export function BrandMark({ size = 24 }: { size?: number }) {
  const id = `routa-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const small = size <= 32;
  const g = small
    ? { rx: 44, orx: 98, ory: 50, w: 22, nodes: [[39.2, 110.9, 34], [212.9, 107, 28], [136.5, 181.8, 44]], far: C.mark }
    : { rx: 58, orx: 100, ory: 44, w: 12, nodes: [[37.4, 113.4, 24], [214.6, 110, 18], [136.7, 175.8, 32]], far: C.farNode };
  return (
    <svg width={size} height={size} viewBox="0 0 256 256" role="img" aria-label="Routa" style={{ display: "block", flexShrink: 0 }}>
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={C.tileTop} />
          <stop offset="1" stopColor={C.tileBottom} />
        </linearGradient>
      </defs>
      <rect width="256" height="256" rx={g.rx} fill={`url(#${id})`} />
      <g transform="rotate(-18 128 132)">
        <ellipse cx="128" cy="132" rx={g.orx} ry={g.ory} fill="none" stroke={C.mark} strokeWidth={g.w} />
        {g.nodes.map(([x, y, r], i) => (
          <circle key={i} cx={x} cy={y} r={r} fill={i === 1 ? g.far : C.mark} />
        ))}
      </g>
    </svg>
  );
}
