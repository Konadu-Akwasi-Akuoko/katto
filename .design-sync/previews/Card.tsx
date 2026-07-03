import * as React from "react";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
  CardAction,
  Button,
} from "katto";

const frame: React.CSSProperties = { padding: 24, maxWidth: 420 };

/** Header, body, and a footer action. */
export const Basic = () => (
  <div style={frame}>
    <Card>
      <CardHeader>
        <CardTitle>Rough Cut · draft-v3</CardTitle>
        <CardDescription>
          AI-assembled from 4 SD cards. 18 clips kept, 6 flagged as filler.
        </CardDescription>
      </CardHeader>
      <CardContent style={{ color: "var(--fg-muted)", fontSize: 13 }}>
        Duration 12:41 · 1080p ProRes · last touched 2 hours ago
      </CardContent>
      <CardFooter style={{ gap: 8 }}>
        <Button size="sm">Open in NLE</Button>
        <Button size="sm" variant="outline">
          Re-cut
        </Button>
      </CardFooter>
    </Card>
  </div>
);

/** A header action pinned to the top-right via CardAction. */
export const WithAction = () => (
  <div style={frame}>
    <Card>
      <CardHeader>
        <CardTitle>Thumbnail set</CardTitle>
        <CardDescription>3 concepts generated · none selected</CardDescription>
        <CardAction>
          <Button size="icon-sm" variant="ghost" aria-label="More">
            <svg viewBox="0 0 16 16" fill="currentColor">
              <circle cx="3" cy="8" r="1.4" />
              <circle cx="8" cy="8" r="1.4" />
              <circle cx="13" cy="8" r="1.4" />
            </svg>
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent style={{ color: "var(--fg-muted)", fontSize: 13 }}>
        Waiting on a human pick — no scoring, no auto-select.
      </CardContent>
    </Card>
  </div>
);
