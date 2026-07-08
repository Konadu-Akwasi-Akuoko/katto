import {
  BellIcon,
  CalendarBlankIcon,
  FilmSlateIcon,
  FilmStripIcon,
  FolderIcon,
  HardDrivesIcon,
  HouseIcon,
  ImageIcon,
  LifebuoyIcon,
  ScissorsIcon,
  ShieldIcon,
  UserIcon,
} from "@phosphor-icons/react";
import type { ComponentType } from "react";

type IconType = ComponentType<{ className?: string }>;
type NavItem = { icon: IconType; label: string; active?: boolean };

const studioNav: NavItem[] = [
  { icon: HouseIcon, label: "Dashboard", active: true },
  { icon: CalendarBlankIcon, label: "Planner" },
  { icon: FilmSlateIcon, label: "Projects" },
  { icon: HardDrivesIcon, label: "Ingest" },
];

const postNav: NavItem[] = [
  { icon: ScissorsIcon, label: "Rough cut" },
  { icon: FilmStripIcon, label: "Timelines" },
  { icon: ImageIcon, label: "Thumbnails" },
  { icon: FolderIcon, label: "Assets" },
];

const accountNav: NavItem[] = [
  { icon: UserIcon, label: "Profile" },
  { icon: BellIcon, label: "Notifications" },
  { icon: ShieldIcon, label: "Security" },
  { icon: LifebuoyIcon, label: "Support" },
];

function NavList({ heading, items }: { heading: string; items: NavItem[] }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="px-2 pb-1 text-xs text-fg-faint">{heading}</span>
      {items.map(({ icon: Icon, label, active }) => (
        <button
          key={label}
          type="button"
          className={`flex items-center gap-2.5 rounded-md px-2 py-1.5 text-sm transition-colors ${
            active
              ? "bg-surface-2 text-fg"
              : "text-fg-muted hover:bg-surface-2 hover:text-fg"
          }`}
        >
          <Icon className="size-4" />
          {label}
        </button>
      ))}
    </div>
  );
}

export function Sidebar() {
  return (
    <aside className="flex w-56 flex-col gap-5 overflow-y-auto border-r p-3 select-none">
      <NavList heading="Studio" items={studioNav} />
      <NavList heading="Post" items={postNav} />
      <div className="mt-auto">
        <NavList heading="Account" items={accountNav} />
      </div>
    </aside>
  );
}
