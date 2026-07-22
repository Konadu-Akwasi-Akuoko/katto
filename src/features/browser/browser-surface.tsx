import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useSonner } from "sonner";
import { Button } from "@/components/ui/button";
import {
	browserCloseTab,
	browserGo,
	browserKeys,
	browserNavigate,
	browserOpenTab,
	browserSelectTab,
	browserSetBounds,
	browserSetVisible,
	browserState,
} from "@/lib/ipc/browser";
import { useDownloadsStore } from "@/stores/downloads";
import { useUiStore } from "@/stores/ui";
import { DownloadsPopover } from "./downloads-popover";
import { NeedsProjectSheet } from "./needs-project-sheet";
import { TabStrip } from "./tab-strip";
import { Toolbar } from "./toolbar";

/**
 * The in-app browser surface. Tabs render as native child webviews layered
 * over the content host div; React owns the chrome (strip, toolbar,
 * downloads) and reports the host rect so the webviews track the layout.
 */
export function BrowserSurface() {
	const contentRef = useRef<HTMLDivElement>(null);
	const openedDefault = useRef(false);
	const [openFailed, setOpenFailed] = useState(false);
	const [popoverOpen, setPopoverOpen] = useState(false);
	const queryClient = useQueryClient();

	// the native child webview paints over the DOM, so every overlay that
	// could cover the page must hide it: palette, dock, downloads popover,
	// needs-project sheet, active toasts
	const paletteOpen = useUiStore((s) => s.paletteOpen);
	const paletteDialog = useUiStore((s) => s.paletteDialog);
	const dockOpen = useUiStore((s) => s.dockOpen);
	const needsProject = useDownloadsStore((s) => s.needsProject);
	const { toasts } = useSonner();
	const overlayOpen =
		paletteOpen ||
		paletteDialog !== null ||
		dockOpen ||
		needsProject !== null ||
		popoverOpen ||
		toasts.length > 0;

	const state = useQuery({
		queryKey: browserKeys.state,
		queryFn: browserState,
	});

	const invalidate = () =>
		queryClient.invalidateQueries({ queryKey: browserKeys.state });

	const openTab = useMutation({
		mutationFn: (url?: string) => browserOpenTab(url),
		onSuccess: () => {
			setOpenFailed(false);
			void invalidate();
		},
		onError: () => setOpenFailed(true),
	});
	const selectTab = useMutation({
		mutationFn: (id: number) => browserSelectTab(id),
		onSuccess: invalidate,
	});
	const closeTab = useMutation({
		mutationFn: (id: number) => browserCloseTab(id),
		onSuccess: invalidate,
	});
	const navigate = useMutation({
		mutationFn: (args: { id: number; url: string }) =>
			browserNavigate(args.id, args.url),
	});
	const go = useMutation({
		mutationFn: (args: { id: number; delta: number }) =>
			browserGo(args.id, args.delta),
		onSuccess: invalidate,
	});

	// surface visibility drives webview visibility; any open overlay hides
	// the page so DOM chrome actually paints above it
	useEffect(() => {
		void browserSetVisible(!overlayOpen);
	}, [overlayOpen]);
	useEffect(() => {
		return () => {
			void browserSetVisible(false);
		};
	}, []);

	// first mount with no tabs: open the Envato default
	const tabs = state.data?.tabs;
	const openMutate = openTab.mutate;
	useEffect(() => {
		if (tabs !== undefined && tabs.length === 0 && !openedDefault.current) {
			openedDefault.current = true;
			openMutate(undefined);
		}
	}, [tabs, openMutate]);

	// report the content-host rect (CSS px = logical px) on every resize
	useEffect(() => {
		const el = contentRef.current;
		if (el === null) return;
		const report = () => {
			const rect = el.getBoundingClientRect();
			void browserSetBounds({
				x: rect.x,
				y: rect.y,
				width: rect.width,
				height: rect.height,
			});
		};
		report();
		const observer = new ResizeObserver(report);
		observer.observe(el);
		window.addEventListener("resize", report);
		return () => {
			observer.disconnect();
			window.removeEventListener("resize", report);
		};
	}, []);

	const active =
		state.data?.tabs.find((t) => t.id === state.data?.active) ?? null;
	const showEmpty = (tabs?.length ?? 0) === 0 && openFailed;

	return (
		<div className="flex h-full min-h-0 flex-col">
			<TabStrip
				tabs={state.data?.tabs ?? []}
				activeId={state.data?.active ?? null}
				onSelect={(id) => selectTab.mutate(id)}
				onClose={(id) => closeTab.mutate(id)}
				onNew={() => openTab.mutate(undefined)}
			/>
			<Toolbar
				activeTab={active}
				onNavigate={(url) => {
					if (active !== null) navigate.mutate({ id: active.id, url });
					else openTab.mutate(url);
				}}
				onGo={(delta) => {
					if (active !== null) go.mutate({ id: active.id, delta });
				}}
			>
				<DownloadsPopover open={popoverOpen} onOpenChange={setPopoverOpen} />
			</Toolbar>
			<div ref={contentRef} className="min-h-0 flex-1">
				{showEmpty && (
					<div className="flex h-full flex-col items-center justify-center gap-2">
						<h2 className="font-serif text-lg text-fg">The web, filed.</h2>
						<p className="max-w-sm text-center text-sm text-fg-muted">
							Downloads from any tab land in the active project's assets folder
							— never in ~/Downloads.
						</p>
						<Button
							size="sm"
							className="mt-2"
							onClick={() => openTab.mutate(undefined)}
						>
							Open Envato Elements
						</Button>
					</div>
				)}
			</div>
			<NeedsProjectSheet />
		</div>
	);
}
