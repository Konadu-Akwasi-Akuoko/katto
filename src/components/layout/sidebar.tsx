import {
	CalendarBlankIcon,
	FilmSlateIcon,
	GearSixIcon,
	GlobeSimpleIcon,
	HouseIcon,
} from "@phosphor-icons/react";
import type { ComponentType, ReactNode } from "react";
import { cn } from "@/lib/utils";
import type { Surface } from "@/stores/ui";
import { useUiStore } from "@/stores/ui";

type IconType = ComponentType<{ className?: string }>;

const studioNav: { surface: Surface; icon: IconType; label: string }[] = [
	{ surface: "dashboard", icon: HouseIcon, label: "Dashboard" },
	{ surface: "planner", icon: CalendarBlankIcon, label: "Planner" },
	{ surface: "projects", icon: FilmSlateIcon, label: "Projects" },
	{ surface: "browser", icon: GlobeSimpleIcon, label: "Browser" },
];

function NavButton({
	surface,
	icon: Icon,
	label,
}: {
	surface: Surface;
	icon: IconType;
	label: string;
}) {
	const active = useUiStore((s) => s.surface) === surface;
	const setSurface = useUiStore((s) => s.setSurface);
	return (
		<button
			type="button"
			aria-current={active ? "page" : undefined}
			onClick={() => setSurface(surface)}
			className={cn(
				"flex items-center gap-2.5 rounded-md px-2 py-1.5 text-sm transition-colors",
				active
					? "bg-surface-2 text-fg"
					: "text-fg-muted hover:bg-surface-2 hover:text-fg",
			)}
		>
			<Icon className="size-4" />
			{label}
		</button>
	);
}

export function Sidebar({ dock }: { dock?: ReactNode }) {
	return (
		<aside className="flex w-56 flex-col gap-5 overflow-y-auto border-r p-3 select-none">
			<div className="flex flex-col gap-0.5">
				<span className="px-2 pb-1 text-xs text-fg-faint">Studio</span>
				{studioNav.map((item) => (
					<NavButton key={item.surface} {...item} />
				))}
			</div>
			<div className="mt-auto flex flex-col gap-0.5">
				<NavButton surface="settings" icon={GearSixIcon} label="Settings" />
				{dock}
			</div>
		</aside>
	);
}
