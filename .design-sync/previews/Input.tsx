import * as React from "react";
import { Input } from "katto";

const col: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 12,
  padding: 24,
  maxWidth: 360,
};

/** Default text field with a domain placeholder. */
export const Default = () => (
  <div style={col}>
    <Input placeholder="Project title — e.g. Studio build tour" />
  </div>
);

/** A filled value alongside the empty placeholder state. */
export const WithValue = () => (
  <div style={col}>
    <Input defaultValue="Rough cut v3 — kitchen segment" />
    <Input placeholder="Untitled timeline" />
  </div>
);

/** Common input types: numeric frame rate and a search field. */
export const Types = () => (
  <div style={col}>
    <Input type="number" defaultValue={24} />
    <Input type="search" placeholder="Search clips…" />
  </div>
);

/** Invalid state via aria-invalid — destructive ring. */
export const Invalid = () => (
  <div style={col}>
    <Input aria-invalid defaultValue="/Volumes/SD_A/missing" />
  </div>
);

/** Disabled field. */
export const Disabled = () => (
  <div style={col}>
    <Input disabled defaultValue="Locked while rendering" />
  </div>
);
