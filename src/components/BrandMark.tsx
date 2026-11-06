import { useId } from "react";
import { BRAND_COLORS as C } from "../brand";

// Routa 应用图标（与 designs/routa-icon-final 的 master.svg / small.svg 一致）。
// 32 px 及以下使用小尺寸版：圆角更小、轨道加粗、节点放大，远处节点也为纯白。
// 图形缩放比例：主图标 0.72、小尺寸版 0.74（与 build.py 的 MASTER_SCALE / SMALL_SCALE 一致）。
export function BrandMark({ size = 24 }: { size?: number }) {
  const id = `routa-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const small = size <= 32;
  const g = small
    ? { rx: 44, scale: 0.74, orx: 98, ory: 50, w: 24, nodes: [[39.2, 110.9, 33], [212.9, 107, 27], [136.5, 181.8, 42]], far: C.mark }
    : { rx: 58, scale: 0.72, orx: 100, ory: 44, w: 14, nodes: [[37.4, 113.4, 25], [214.6, 110, 19], [136.7, 175.8, 33]], far: C.farNode };
  return (
    <svg width={size} height={size} viewBox="0 0 256 256" role="img" aria-label="Routa" style={{ display: "block", flexShrink: 0 }}>
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={C.tileTop} />
          <stop offset="1" stopColor={C.tileBottom} />
        </linearGradient>
      </defs>
      <rect width="256" height="256" rx={g.rx} fill={`url(#${id})`} />
      {/* 图形以视觉中心 (122, 135) 缩放后居中，留白比例参考钉钉 */}
      <g transform={`translate(128 128) scale(${g.scale}) translate(-122 -135) rotate(-18 128 132)`}>
        <ellipse cx="128" cy="132" rx={g.orx} ry={g.ory} fill="none" stroke={C.mark} strokeWidth={g.w} />
        {g.nodes.map(([x, y, r], i) => (
          <circle key={i} cx={x} cy={y} r={r} fill={i === 1 ? g.far : C.mark} />
        ))}
      </g>
    </svg>
  );
}
