import * as React from "react";
import { Calendar } from "katto";

const frame: React.CSSProperties = { padding: 24, width: "fit-content" };

// July 2026 — katto's "today" (the 3rd) lands in view, so the today marker shows
// alongside the ember-selected day.
const july = new Date(2026, 6, 1);

/** Single-date selection — the picked day fills with ember. */
export const Single = () => {
  const [selected, setSelected] = React.useState<Date | undefined>(
    new Date(2026, 6, 15)
  );
  return (
    <div style={frame}>
      <Calendar
        mode="single"
        defaultMonth={july}
        selected={selected}
        onSelect={setSelected}
        className="rounded-lg border"
      />
    </div>
  );
};

/** Range selection — start and end fill ember, the span sits on surface. */
export const Range = () => {
  const [range, setRange] = React.useState<
    { from: Date | undefined; to: Date | undefined } | undefined
  >({ from: new Date(2026, 6, 8), to: new Date(2026, 6, 14) });
  return (
    <div style={frame}>
      <Calendar
        mode="range"
        defaultMonth={july}
        selected={range}
        onSelect={setRange}
        className="rounded-lg border"
      />
    </div>
  );
};

/** Month and year as dropdowns instead of a static caption. */
export const Dropdowns = () => {
  const [selected, setSelected] = React.useState<Date | undefined>(
    new Date(2026, 6, 3)
  );
  return (
    <div style={frame}>
      <Calendar
        mode="single"
        captionLayout="dropdown"
        defaultMonth={july}
        startMonth={new Date(2024, 0)}
        endMonth={new Date(2027, 11)}
        selected={selected}
        onSelect={setSelected}
        className="rounded-lg border"
      />
    </div>
  );
};
