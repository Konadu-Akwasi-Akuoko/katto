import * as React from "react";
import {
  Command,
  CommandInput,
  CommandList,
  CommandEmpty,
  CommandGroup,
  CommandItem,
  CommandShortcut,
  CommandSeparator,
} from "katto";

const frame: React.CSSProperties = { padding: 24, maxWidth: 460 };

const palette: React.CSSProperties = {
  width: 420,
  border: "1px solid var(--border)",
  borderRadius: 12,
  boxShadow: "0 12px 32px rgba(0,0,0,0.18)",
  overflow: "hidden",
};

/** The studio command palette — grouped actions with keyboard shortcuts. */
export const StudioPalette = () => (
  <div style={frame}>
    <div style={palette}>
      <Command>
        <CommandInput placeholder="Run a studio command…" />
        <CommandList>
          <CommandEmpty>No matching command.</CommandEmpty>
          <CommandGroup heading="Capture">
            <CommandItem>
              Import SD card
              <CommandShortcut>⌘I</CommandShortcut>
            </CommandItem>
            <CommandItem>
              Detect new footage
              <CommandShortcut>⌘D</CommandShortcut>
            </CommandItem>
          </CommandGroup>
          <CommandSeparator />
          <CommandGroup heading="Edit">
            <CommandItem>
              Start rough cut
              <CommandShortcut>⌘R</CommandShortcut>
            </CommandItem>
            <CommandItem>
              Export timeline to NLE
              <CommandShortcut>⌘E</CommandShortcut>
            </CommandItem>
            <CommandItem>
              Generate thumbnails
              <CommandShortcut>⌘T</CommandShortcut>
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </Command>
    </div>
  </div>
);

/** Compact recent-commands palette — a single group, no shortcuts. */
export const Recent = () => (
  <div style={frame}>
    <div style={palette}>
      <Command>
        <CommandInput placeholder="Search commands…" />
        <CommandList>
          <CommandEmpty>No matching command.</CommandEmpty>
          <CommandGroup heading="Recent">
            <CommandItem>Re-cut draft-v3</CommandItem>
            <CommandItem>Open thumbnail set</CommandItem>
            <CommandItem>Export timeline to NLE</CommandItem>
          </CommandGroup>
        </CommandList>
      </Command>
    </div>
  </div>
);
