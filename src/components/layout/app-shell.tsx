import type { ReactNode } from "react";

/**
 * The desktop app shell. The window itself never scrolls: this is a fixed
 * 100dvh grid (titlebar row / body row), and the body splits into a fixed
 * sidebar and a single scrollable content pane. `min-h-0` on the grid tracks
 * is what lets the content pane actually scroll instead of growing the frame.
 */
export function AppShell({
  titlebar,
  sidebar,
  children,
}: {
  titlebar: ReactNode;
  sidebar: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="grid h-dvh grid-rows-[auto_1fr] overflow-hidden">
      {titlebar}
      <div className="grid min-h-0 grid-cols-[auto_1fr]">
        {sidebar}
        <main
          data-scroll-root
          className="min-h-0 overflow-y-auto overscroll-contain"
        >
          {children}
        </main>
      </div>
    </div>
  );
}
