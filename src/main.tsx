import React from "react";
import ReactDOM from "react-dom/client";
import App from "@/app/app";
import { applyTheme, storedTheme } from "@/lib/theme";
import { windowLabel } from "@/lib/window-label";
import "@/styles/main.css";

// The WebView is destroyed on close-to-tray; restore the theme before first
// paint so a reopened window doesn't flash the index.html default.
applyTheme(storedTheme());

// Lets the stylesheet strip the page background on the transparent capture
// frame. Set before first paint, or the app ground flashes behind its corners.
document.documentElement.dataset.window = windowLabel();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
	<React.StrictMode>
		<App />
	</React.StrictMode>,
);
