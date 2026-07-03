import * as React from "react";
import { Switch, Label } from "katto";

const frame: React.CSSProperties = {
  padding: 24,
  maxWidth: 380,
  display: "flex",
  flexDirection: "column",
  gap: 18,
};

const row: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: 24,
};

const hint: React.CSSProperties = { color: "var(--fg-muted)", fontSize: 12, marginTop: 2 };

/** Real ingest settings rows — on, off, and a locked/disabled option. */
export const IngestSettings = () => (
  <div style={frame}>
    <div style={row}>
      <div>
        <Label htmlFor="s-backup">Back up SD card on import</Label>
        <div style={hint}>Copy raw footage to the archive drive first</div>
      </div>
      <Switch id="s-backup" defaultChecked />
    </div>
    <div style={row}>
      <div>
        <Label htmlFor="s-proxy">Generate proxy media</Label>
        <div style={hint}>Skip for short clips under 2 minutes</div>
      </div>
      <Switch id="s-proxy" />
    </div>
    <div style={row}>
      <div>
        <Label htmlFor="s-eject">Auto-eject after copy</Label>
        <div style={hint}>Locked while a copy is running</div>
      </div>
      <Switch id="s-eject" defaultChecked disabled />
    </div>
  </div>
);

/** Both sizes side by side — default and sm. */
export const Sizes = () => (
  <div style={{ padding: 24, display: "flex", alignItems: "center", gap: 20 }}>
    <Switch defaultChecked />
    <Switch />
    <Switch size="sm" defaultChecked />
    <Switch size="sm" />
  </div>
);
