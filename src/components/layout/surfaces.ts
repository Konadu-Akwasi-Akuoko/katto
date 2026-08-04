import {
	CalendarBlankIcon,
	FilmSlateIcon,
	GearSixIcon,
	GlobeSimpleIcon,
	HouseIcon,
} from "@phosphor-icons/react";
import type { ComponentType } from "react";
import type { Surface } from "@/stores/ui";

export type SurfaceIcon = ComponentType<{ className?: string }>;

export type SurfaceNavItem = {
	surface: Surface;
	icon: SurfaceIcon;
	label: string;
};

/**
 * Every navigable surface, in sidebar order. Shared so the sidebar rail and the
 * titlebar switcher cannot drift apart — they are two views of one list.
 */
export const SURFACE_NAV: SurfaceNavItem[] = [
	{ surface: "dashboard", icon: HouseIcon, label: "Dashboard" },
	{ surface: "planner", icon: CalendarBlankIcon, label: "Planner" },
	{ surface: "projects", icon: FilmSlateIcon, label: "Projects" },
	{ surface: "browser", icon: GlobeSimpleIcon, label: "Browser" },
	{ surface: "settings", icon: GearSixIcon, label: "Settings" },
];

/** The four studio surfaces; Settings sits apart at the foot of the sidebar. */
export const STUDIO_NAV = SURFACE_NAV.filter((s) => s.surface !== "settings");
