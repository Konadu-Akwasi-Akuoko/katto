import * as React from "react";
import { Button } from "katto";

const row: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  alignItems: "center",
  gap: 10,
  padding: 24,
};

/** The six variants, ember-primary first. */
export const Variants = () => (
  <div style={row}>
    <Button>Render</Button>
    <Button variant="secondary">Preview</Button>
    <Button variant="outline">Import SD</Button>
    <Button variant="ghost">Skip</Button>
    <Button variant="destructive">Discard cut</Button>
    <Button variant="link">View timeline</Button>
  </div>
);

/** Size scale from xs to lg. */
export const Sizes = () => (
  <div style={row}>
    <Button size="xs">xs</Button>
    <Button size="sm">Small</Button>
    <Button>Default</Button>
    <Button size="lg">Large</Button>
  </div>
);

/** Leading icon — svg auto-sizes to the control. */
export const WithIcon = () => (
  <div style={row}>
    <Button>
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.6}>
        <path d="M4 3.5v9l7-4.5-7-4.5Z" fill="currentColor" stroke="none" />
      </svg>
      Start render
    </Button>
    <Button variant="outline" size="icon" aria-label="Settings">
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.4}>
        <circle cx="8" cy="8" r="2.2" />
        <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M12.6 3.4l-1.4 1.4M4.8 11.2l-1.4 1.4" />
      </svg>
    </Button>
  </div>
);

/** Disabled state. */
export const Disabled = () => (
  <div style={row}>
    <Button disabled>Render</Button>
    <Button variant="outline" disabled>
      Import SD
    </Button>
  </div>
);
