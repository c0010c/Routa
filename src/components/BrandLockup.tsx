import { useId } from "react";
import { BRAND_COLORS as C } from "../brand";

// Routa 横版标志（裸符号 + 字标），与 designs/routa-lockup/svg/bare-light.svg 一致。
// 字标由等粗圆头笔画构成，不依赖字体；height 为整体高度，宽度按比例（713 : 206）自动计算。
const WORD_PATHS = [
  "M11 200 V70 H52 A35 35 0 0 1 52 140 H11 M46 140 L86 200",
  "M193 155 A34 34 0 1 1 125 155 A34 34 0 1 1 193 155",
  "M229 110 V160 A34 34 0 0 0 297 160 V110 M297 110 V200",
  "M346 80 V176 Q346 200 370 200 M324 110 H368",
  "M465 155 A34 34 0 1 1 397 155 A34 34 0 1 1 465 155 M465 110 V200",
];

export function BrandLockup({ height = 28 }: { height?: number }) {
  const id = useId();
  return (
    <svg
      height={height}
      width={(height * 713) / 206}
      viewBox="0 0 713 206"
      role="img"
      aria-label="Routa"
      style={{ display: "block", flexShrink: 0 }}
    >
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor={C.tileTop} />
          <stop offset="1" stopColor={C.tileBottom} />
        </linearGradient>
      </defs>
      <g transform="translate(14 -32) rotate(-18 128 132)">
        <ellipse cx="128" cy="132" rx="100" ry="44" fill="none" stroke={`url(#${id})`} strokeWidth={14} />
        <circle cx="37.4" cy="113.4" r="24" fill={C.tileBottom} />
        <circle cx="214.6" cy="110" r="18" fill={C.symbolFarNode} />
        <circle cx="136.7" cy="175.8" r="32" fill={C.tileTop} />
      </g>
      <g
        transform="translate(294 -9) scale(0.83)"
        fill="none"
        stroke={C.wordmark}
        strokeWidth={22}
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        {WORD_PATHS.map((d) => (
          <path key={d} d={d} />
        ))}
      </g>
    </svg>
  );
}
