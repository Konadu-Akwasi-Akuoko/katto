import * as React from "react";
import { Separator } from "katto";

const frame: React.CSSProperties = { padding: 24, maxWidth: 360 };

/** Horizontal rule splitting stacked metadata. */
export const Horizontal = () => (
  <div style={frame}>
    <div style={{ fontSize: 14, color: "var(--fg)" }}>Rough cut v3</div>
    <div style={{ fontSize: 13, color: "var(--fg-muted)", marginBottom: 12 }}>
      12:41 · 1080p ProRes
    </div>
    <Separator />
    <div style={{ fontSize: 13, color: "var(--fg-muted)", marginTop: 12 }}>
      Last touched 2 hours ago
    </div>
  </div>
);

/** Vertical dividers between inline items — parent gets a fixed height. */
export const Vertical = () => (
  <div
    style={{
      display: "flex",
      alignItems: "center",
      gap: 12,
      height: 24,
      padding: 24,
      fontSize: 13,
      color: "var(--fg-muted)",
    }}
  >
    <span>18 clips</span>
    <Separator orientation="vertical" />
    <span>6 flagged</span>
    <Separator orientation="vertical" />
    <span>4 SD cards</span>
  </div>
);
