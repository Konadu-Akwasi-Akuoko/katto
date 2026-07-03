import {
  ArrowRightIcon,
  BellIcon,
  CalendarBlankIcon,
  FilmSlateIcon,
  FilmStripIcon,
  FolderIcon,
  GearSixIcon,
  HardDrivesIcon,
  HouseIcon,
  ImageIcon,
  LifebuoyIcon,
  MagnifyingGlassIcon,
  MoonIcon,
  PlusIcon,
  RobotIcon,
  ScissorsIcon,
  ShieldIcon,
  SunIcon,
  UserIcon,
  WaveformIcon,
} from "@phosphor-icons/react";
import type { ComponentType } from "react";
import { useState } from "react";
import { toast } from "sonner";
import { AppShell } from "@/components/layout/app-shell";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Command,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Toaster } from "@/components/ui/sonner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

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

const feed: [string, string, boolean][] = [
  ["Rough cut started on a-roll.mov", "just now", true],
  ["Ingested 142 clips from CFexpress card", "12:04", false],
  ["Export v2 opened in Final Cut", "11:38", false],
  ["Studio root reconnected", "09:20", false],
  ["Thumbnail exported for alps-day-2", "Yesterday", false],
];

export default function App() {
  const [dark, setDark] = useState(true);

  function toggleTheme() {
    const root = document.documentElement;
    root.classList.toggle("dark");
    setDark(root.classList.contains("dark"));
  }

  const titlebar = (
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
            <Button variant="ghost" size="icon" onClick={toggleTheme}>
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

  const sidebar = (
    <aside className="flex w-56 flex-col gap-5 overflow-y-auto border-r p-3 select-none">
      <NavList heading="Studio" items={studioNav} />
      <NavList heading="Post" items={postNav} />
      <div className="mt-auto">
        <NavList heading="Account" items={accountNav} />
      </div>
    </aside>
  );

  return (
    <TooltipProvider delayDuration={200}>
      <AppShell titlebar={titlebar} sidebar={sidebar}>
        <div className="columns-1 gap-4 p-4 md:columns-2 xl:columns-3 [&>*]:mb-4 [&>*]:break-inside-avoid">
          <Card>
            <CardContent className="flex flex-col gap-4">
              <div className="flex flex-wrap gap-2">
                <Button className="gap-1.5">
                  Button <ArrowRightIcon className="size-4" />
                </Button>
                <Button variant="secondary">Secondary</Button>
                <Button variant="outline">Outline</Button>
                <Button variant="ghost">Ghost</Button>
              </div>
              <div className="flex flex-col gap-1.5">
                <Label htmlFor="name">Project title</Label>
                <Input id="name" placeholder="alps-day-3" />
              </div>
              <div className="flex flex-col gap-1.5">
                <Label htmlFor="notes">Notes</Label>
                <textarea
                  id="notes"
                  rows={3}
                  placeholder="Shot list, gear, locations…"
                  className="rounded-md border bg-surface px-3 py-2 text-sm outline-none placeholder:text-fg-faint focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                />
              </div>
              <div className="flex items-center justify-between">
                <div className="flex gap-2">
                  <Badge>Badge</Badge>
                  <Badge variant="secondary">Secondary</Badge>
                  <Badge variant="outline">Outline</Badge>
                </div>
                <Switch defaultChecked />
              </div>
              <Dialog>
                <DialogTrigger asChild>
                  <Button variant="secondary">Open dialog</Button>
                </DialogTrigger>
                <DialogContent>
                  <DialogHeader>
                    <DialogTitle>Discard rough cut?</DialogTitle>
                    <DialogDescription>
                      The AI cut for alps-day-3 will be removed. Your source clips stay untouched.
                    </DialogDescription>
                  </DialogHeader>
                  <DialogFooter>
                    <Button variant="ghost">Keep it</Button>
                    <Button variant="destructive">Discard</Button>
                  </DialogFooter>
                </DialogContent>
              </Dialog>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Running now</CardTitle>
              <CardDescription>Nothing runs invisibly</CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <div className="flex flex-col gap-2">
                <div className="flex items-center justify-between text-sm">
                  <span>Rough cut · a-roll.mov</span>
                  <Badge variant="running">
                    <span className="dot" />
                    running
                  </Badge>
                </div>
                <Progress value={64} />
                <span className="font-mono text-xs text-fg-faint tabular-nums">
                  64% · 00:04:12:18
                </span>
              </div>
              <Separator />
              <div className="flex items-center justify-between text-sm">
                <span>Transcribe · interview_02.wav</span>
                <Badge variant="queued">
                  <span className="dot" />
                  queued
                </Badge>
              </div>
              <div className="flex items-center justify-between text-sm">
                <span>Export · alps-day-3-v3.fcpxml</span>
                <Badge variant="failed">
                  <span className="dot" />
                  failed
                </Badge>
              </div>
              <div className="flex items-center justify-between text-sm">
                <span>Ingest · CFexpress card</span>
                <Badge variant="done">
                  <span className="dot" />
                  done
                </Badge>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Set a new shoot</CardTitle>
              <CardDescription>Plan the next production day.</CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <div className="flex flex-col gap-1.5">
                <Label htmlFor="goal">Working title</Label>
                <Input id="goal" placeholder="e.g. Alps day 3 — summit push" />
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div className="flex flex-col gap-1.5">
                  <Label htmlFor="fmt">Format</Label>
                  <Select defaultValue="long">
                    <SelectTrigger id="fmt">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="long">Long-form</SelectItem>
                      <SelectItem value="short">Short</SelectItem>
                      <SelectItem value="series">Series</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div className="flex flex-col gap-1.5">
                  <Label htmlFor="date">Shoot date</Label>
                  <Input id="date" defaultValue="Sat 11 Jul" />
                </div>
              </div>
              <div className="flex flex-col gap-2">
                <Button className="w-full">Create project</Button>
                <Button variant="ghost" className="w-full">
                  Cancel
                </Button>
              </div>
            </CardContent>
          </Card>

          <Card className="overflow-hidden p-0">
            <Command className="bg-transparent">
              <CommandInput placeholder="Type a command…" />
              <CommandList>
                <CommandGroup heading="Project">
                  <CommandItem>
                    <FilmSlateIcon className="size-4" />
                    Export current timeline
                  </CommandItem>
                  <CommandItem>
                    <FolderIcon className="size-4" />
                    Open in Final Cut
                  </CommandItem>
                  <CommandItem>
                    <WaveformIcon className="size-4" />
                    Clean audio
                  </CommandItem>
                </CommandGroup>
                <CommandSeparator />
                <CommandGroup heading="System">
                  <CommandItem>
                    <RobotIcon className="size-4" />
                    Re-run claude detection
                  </CommandItem>
                  <CommandItem>
                    <GearSixIcon className="size-4" />
                    Open settings
                  </CommandItem>
                </CommandGroup>
              </CommandList>
            </Command>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Studio SSD</CardTitle>
              <CardDescription>/Volumes/KATTO-SSD</CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
              <div className="flex items-baseline gap-2">
                <span className="font-serif text-4xl font-semibold tabular-nums">1.2</span>
                <span className="text-fg-muted">TB free of 4 TB</span>
              </div>
              <Progress value={70} />
              <Separator />
              <div className="flex flex-col gap-2 font-mono text-xs tabular-nums">
                <div className="flex justify-between text-fg-muted">
                  <span>Footage</span>
                  <span className="text-fg">2.1 TB</span>
                </div>
                <div className="flex justify-between text-fg-muted">
                  <span>Renders</span>
                  <span className="text-fg">0.7 TB</span>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Settings</CardTitle>
            </CardHeader>
            <CardContent>
              <Tabs defaultValue="general">
                <TabsList>
                  <TabsTrigger value="general">General</TabsTrigger>
                  <TabsTrigger value="ai">AI</TabsTrigger>
                  <TabsTrigger value="keys">Keys</TabsTrigger>
                </TabsList>
                <TabsContent value="general" className="flex flex-col gap-3 pt-3">
                  <div className="flex items-center justify-between text-sm">
                    <span>Launch at login</span>
                    <Switch defaultChecked />
                  </div>
                  <div className="flex items-center justify-between text-sm">
                    <span>Reduce motion</span>
                    <Switch />
                  </div>
                </TabsContent>
                <TabsContent value="ai" className="pt-3 text-sm text-fg-muted">
                  Cut planning routes through a visible Claude session.
                </TabsContent>
                <TabsContent value="keys" className="pt-3 text-sm text-fg-muted">
                  ElevenLabs and Anthropic keys live in the macOS keychain.
                </TabsContent>
              </Tabs>
              <Separator className="my-4" />
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="secondary"
                  onClick={() =>
                    toast.success("Export finished", {
                      description: "alps-day-3-v3.fcpxml",
                    })
                  }
                >
                  Success toast
                </Button>
                <Button
                  variant="secondary"
                  onClick={() =>
                    toast.error("Render failed", {
                      description: "ffmpeg exited with code 1",
                    })
                  }
                >
                  Error toast
                </Button>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Activity</CardTitle>
            </CardHeader>
            <CardContent className="p-0">
              <ScrollArea className="h-44">
                <div className="flex flex-col gap-3 px-6 pb-4 text-sm">
                  {feed.map(([text, when, ember]) => (
                    <div key={text} className="flex items-baseline gap-2.5">
                      <span
                        className={`mt-1.5 size-1.5 shrink-0 rounded-full ${
                          ember ? "bg-ember" : "bg-fg-faint"
                        }`}
                      />
                      <span className="flex-1">{text}</span>
                      <span className="font-mono text-xs text-fg-faint tabular-nums">
                        {when}
                      </span>
                    </div>
                  ))}
                </div>
              </ScrollArea>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Loading</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
              <div className="flex items-center gap-3">
                <Skeleton className="size-10 rounded-md" />
                <div className="flex flex-1 flex-col gap-2">
                  <Skeleton className="h-3 w-2/3" />
                  <Skeleton className="h-3 w-1/3" />
                </div>
              </div>
              <Skeleton className="h-24 w-full rounded-md" />
            </CardContent>
          </Card>

          <Card
            className="border-warn/40 bg-warn/10"
            style={{ backgroundImage: "none" }}
          >
            <CardContent className="flex items-center gap-3">
              <span className="size-2 shrink-0 rounded-full bg-warn" />
              <div className="flex-1 text-sm">
                <div>Studio drive disconnected</div>
                <div className="font-mono text-xs text-fg-muted">/Volumes/KATTO-SSD</div>
              </div>
              <Button variant="ghost" size="sm">
                Retry
              </Button>
            </CardContent>
          </Card>
        </div>
      </AppShell>
      <Toaster theme={dark ? "dark" : "light"} position="bottom-right" />
    </TooltipProvider>
  );
}
