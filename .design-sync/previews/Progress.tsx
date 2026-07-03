import * as React from "react";
import { Progress } from "katto";

const frame: React.CSSProperties = {
  padding: 24,
  maxWidth: 420,
  display: "flex",
  flexDirection: "column",
  gap: 20,
};

const rowLabel: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  fontSize: 13,
  marginBottom: 8,
  color: "var(--fg)",
};

const pct: React.CSSProperties = { color: "var(--fg-muted)", fontVariantNumeric: "tabular-nums" };

/** A render job progressing through several stages, each with a labelled bar. */
export const RenderStages = () => (
  <div style={frame}>
    <div>
      <div style={rowLabel}>
        <span>Ingesting SD card A</span>
        <span style={pct}>100%</span>
      </div>
      <Progress value={100} />
    </div>
    <div>
      <div style={rowLabel}>
        <span>Transcribing audio</span>
        <span style={pct}>62%</span>
      </div>
      <Progress value={62} />
    </div>
    <div>
      <div style={rowLabel}>
        <span>Exporting 1080p ProRes</span>
        <span style={pct}>0%</span>
      </div>
      <Progress value={0} />
    </div>
  </div>
);

/** Single wide bar for a rough-cut render, mid-progress. */
export const RoughCut = () => (
  <div style={frame}>
    <div style={rowLabel}>
      <span>Assembling rough cut · draft-v3</span>
      <span style={pct}>38%</span>
    </div>
    <Progress value={38} />
  </div>
);
