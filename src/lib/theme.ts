const STORAGE_KEY = "katto-theme";

export type Theme = "dark" | "light";

/**
 * The persisted theme preference. Dark is katto's primary look, so anything
 * other than an explicit "light" reads as dark.
 */
export function storedTheme(): Theme {
	return localStorage.getItem(STORAGE_KEY) === "light" ? "light" : "dark";
}

/**
 * Apply a theme to the document root and persist the choice. Called before
 * React mounts (main.tsx) so a re-created WebView paints the right theme,
 * and from the titlebar toggle thereafter.
 */
export function applyTheme(theme: Theme): void {
	document.documentElement.classList.toggle("dark", theme === "dark");
	localStorage.setItem(STORAGE_KEY, theme);
}
