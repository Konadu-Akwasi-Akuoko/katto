import * as React from "react";
import { Label, Input } from "katto";

const field: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  padding: 24,
  maxWidth: 360,
};

/** The canonical label + field pair. */
export const WithInput = () => (
  <div style={field}>
    <Label htmlFor="title">Project title</Label>
    <Input id="title" placeholder="Studio build tour" />
  </div>
);

/** Two stacked labelled fields, as in a form. */
export const FormRows = () => (
  <div style={{ ...field, gap: 14 }}>
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <Label htmlFor="dest">Export destination</Label>
      <Input id="dest" defaultValue="~/Studio/Exports" />
    </div>
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <Label htmlFor="fps">Frame rate</Label>
      <Input id="fps" type="number" defaultValue={24} />
    </div>
  </div>
);

/** Disabled field dims its associated label via peer state. */
export const DisabledField = () => (
  <div style={field}>
    <Label htmlFor="locked" className="peer-disabled:opacity-50">
      Source folder
    </Label>
    <Input id="locked" className="peer" disabled defaultValue="Locked while ingesting" />
  </div>
);
