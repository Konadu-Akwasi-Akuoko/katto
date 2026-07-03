import * as React from "react";
import { Skeleton } from "katto";

const frame: React.CSSProperties = { padding: 24, maxWidth: 360 };

/** Stacked text lines mimicking a loading paragraph. */
export const TextLines = () => (
  <div style={{ ...frame, display: "flex", flexDirection: "column", gap: 10 }}>
    <Skeleton style={{ height: 12, width: "70%" }} />
    <Skeleton style={{ height: 12, width: "100%" }} />
    <Skeleton style={{ height: 12, width: "85%" }} />
  </div>
);

/** A loading media card: thumbnail, title line, meta line. */
export const MediaCard = () => (
  <div style={{ ...frame, display: "flex", gap: 14, alignItems: "center" }}>
    <Skeleton style={{ height: 56, width: 96, borderRadius: 8, flexShrink: 0 }} />
    <div style={{ display: "flex", flexDirection: "column", gap: 8, flex: 1 }}>
      <Skeleton style={{ height: 12, width: "60%" }} />
      <Skeleton style={{ height: 10, width: "40%" }} />
    </div>
  </div>
);

/** Avatar circle plus a two-line caption — a loading list row. */
export const AvatarRow = () => (
  <div style={{ ...frame, display: "flex", gap: 12, alignItems: "center" }}>
    <Skeleton style={{ height: 40, width: 40, borderRadius: 999, flexShrink: 0 }} />
    <div style={{ display: "flex", flexDirection: "column", gap: 8, flex: 1 }}>
      <Skeleton style={{ height: 12, width: "50%" }} />
      <Skeleton style={{ height: 10, width: "75%" }} />
    </div>
  </div>
);
