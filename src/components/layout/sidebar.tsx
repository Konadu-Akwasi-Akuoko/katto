import { GearSixIcon } from "@phosphor-icons/react";
import type { ReactNode } from "react";
import { STUDIO_NAV, type SurfaceIcon } from "@/components/layout/surfaces";
import { cn } from "@/lib/utils";
import type { Surface } from "@/stores/ui";
import { useUiStore } from "@/stores/ui";

type IconType = SurfaceIcon;

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
	const collapsed = useUiStore((s) => s.sidebarCollapsed);
	// Collapsed means gone, not narrow: the titlebar switcher already names the
	// current surface, so an icon rail would be a second, redundant indicator
	// still charging rent on the page width the browser needs.
	if (collapsed) return null;
	return (
		<aside className="flex w-56 shrink-0 flex-col gap-5 overflow-y-auto border-r p-3 select-none">
			<div className="flex flex-col gap-0.5">
				<span className="px-2 pb-1 text-xs text-fg-faint">Studio</span>
				{STUDIO_NAV.map((item) => (
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
