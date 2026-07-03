import * as React from "react";
import { Badge } from "katto";

const row: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  alignItems: "center",
  gap: 10,
  padding: 24,
};

/** The base shadcn variants, ember-primary first. */
export const Variants = () => (
  <div style={row}>
    <Badge>Selected</Badge>
    <Badge variant="secondary">Draft</Badge>
    <Badge variant="destructive">Corrupt</Badge>
    <Badge variant="outline">Optional</Badge>
  </div>
);

/** katto job/state chips — semantic colour, no accent rail. */
export const JobStates = () => (
  <div style={row}>
    <Badge variant="running">Rendering</Badge>
    <Badge variant="queued">Queued</Badge>
    <Badge variant="done">Exported</Badge>
    <Badge variant="failed">Ingest failed</Badge>
  </div>
);

/** A leading dot marks live status; svg icons auto-size to 12px. */
export const WithDot = () => (
  <div style={row}>
    <Badge variant="running">
      <span className="dot" />
      SD card A · copying
    </Badge>
    <Badge variant="done">
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={2}>
        <path d="M3.5 8.5l3 3 6-7" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
      1080p ProRes
    </Badge>
  </div>
);

/** In-context: chips trailing a clip label. */
export const InContext = () => (
  <div style={{ ...row, gap: 8, fontSize: 14, color: "var(--fg)" }}>
    <span>Rough cut v3</span>
    <Badge variant="secondary">18 clips</Badge>
    <Badge variant="queued">Awaiting review</Badge>
  </div>
);
