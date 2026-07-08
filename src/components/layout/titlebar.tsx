import {
  MagnifyingGlassIcon,
  MoonIcon,
  PlusIcon,
  SunIcon,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

export function Titlebar({
  dark,
  onToggleTheme,
}: {
  dark: boolean;
  onToggleTheme: () => void;
}) {
  return (
    <header className="flex items-center gap-3 border-b px-4 py-2.5 select-none">
      <span className="font-serif text-lg leading-none font-semibold">katto</span>
      <span className="text-xs text-fg-faint">alps-day-3</span>
      <div className="ml-auto flex items-center gap-2">
        <div className="relative hidden sm:block">
          <MagnifyingGlassIcon className="absolute top-2.5 left-2.5 size-4 text-fg-faint" />
          <Input placeholder="Search or press ⌘K" className="h-9 w-64 pl-8" />
        </div>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" onClick={onToggleTheme}>
              {dark ? <SunIcon className="size-4" /> : <MoonIcon className="size-4" />}
            </Button>
          </TooltipTrigger>
          <TooltipContent>Toggle theme</TooltipContent>
        </Tooltip>
        <Button className="gap-1.5">
          <PlusIcon className="size-4" />
          New project
        </Button>
      </div>
    </header>
  );
}
