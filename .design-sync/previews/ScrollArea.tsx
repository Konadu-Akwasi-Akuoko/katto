import * as React from "react";
import { ScrollArea } from "katto";

const frame: React.CSSProperties = { padding: 24, maxWidth: 360 };

const shell: React.CSSProperties = {
  height: 220,
  width: 300,
  border: "1px solid var(--border)",
  borderRadius: 10,
  background: "var(--surface)",
};

const heading: React.CSSProperties = {
  padding: "10px 14px",
  fontSize: 12,
  fontWeight: 600,
  color: "var(--fg-muted)",
  borderBottom: "1px solid var(--border)",
};

const rowStyle: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  padding: "8px 14px",
  fontSize: 13,
  color: "var(--fg)",
  borderBottom: "1px solid var(--border)",
};

const dur: React.CSSProperties = { color: "var(--fg-muted)", fontVariantNumeric: "tabular-nums" };

const clips = Array.from({ length: 22 }, (_, i) => ({
  name: `Clip ${String(i + 1).padStart(2, "0")} — C0${100 + i}.MP4`,
  dur: `0:${String((i * 7) % 60).padStart(2, "0")}`,
}));

/** A long clip list overflowing a fixed-height scroll region. */
export const ClipList = () => (
  <div style={frame}>
    <div style={shell}>
      <div style={heading}>Rough cut · 22 clips</div>
      <ScrollArea style={{ height: 220 - 39 }}>
        <div>
          {clips.map((c) => (
            <div key={c.name} style={rowStyle}>
              <span>{c.name}</span>
              <span style={dur}>{c.dur}</span>
            </div>
          ))}
        </div>
      </ScrollArea>
    </div>
  </div>
);
