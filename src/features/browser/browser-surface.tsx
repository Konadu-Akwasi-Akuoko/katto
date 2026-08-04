import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useSonner } from "sonner";
import {
	browserCloseTab,
	browserGo,
	browserKeys,
	browserNavigate,
	browserOpenTab,
	browserSelectTab,
	browserSetVisible,
	browserState,
} from "@/lib/ipc/browser";
import { useDownloadsStore } from "@/stores/downloads";
import { useUiStore } from "@/stores/ui";
import { DownloadsButton, DownloadsPanel } from "./downloads-panel";
import { useBrowserBounds } from "./hooks/use-browser-bounds";
import { NeedsProjectSheet } from "./needs-project-sheet";
import { StartPage } from "./start-page";
import { TabStrip } from "./tab-strip";
import { Toolbar } from "./toolbar";

/**
 * The in-app browser surface. Tabs render as native child webviews layered
 * over the content host div; React owns the chrome (strip, toolbar,
 * downloads) and reports the host rect so the webviews track the layout.
 * A tab holding no URL owns no webview, so the start page paints through.
 */
export function BrowserSurface() {
	const contentRef = useRef<HTMLDivElement>(null);
	const [downloadsOpen, setDownloadsOpen] = useState(false);
	const queryClient = useQueryClient();

	// the native child webview paints over the DOM, so every overlay that
	// could cover the page must hide it: palette, dock, needs-project sheet,
	// active toasts. The downloads panel is deliberately absent — it takes
	// width from the content host rather than covering it, so the page stays
	// live beside it.
	const paletteOpen = useUiStore((s) => s.paletteOpen);
	const paletteDialog = useUiStore((s) => s.paletteDialog);
	const dockOpen = useUiStore((s) => s.dockOpen);
	const switcherOpen = useUiStore((s) => s.switcherOpen);
	const needsProject = useDownloadsStore((s) => s.needsProject);
	const { toasts } = useSonner();
	const overlayOpen =
		paletteOpen ||
		paletteDialog !== null ||
		dockOpen ||
		switcherOpen ||
		needsProject !== null ||
		toasts.length > 0;

	const state = useQuery({
		queryKey: browserKeys.state,
		queryFn: browserState,
	});

	const invalidate = () =>
		queryClient.invalidateQueries({ queryKey: browserKeys.state });

	const openTab = useMutation({
		mutationFn: (url?: string) => browserOpenTab(url),
		onSuccess: invalidate,
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

	const remeasure = useBrowserBounds(contentRef);

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

	// the host moves without resizing whenever chrome above it changes, and a
	// ResizeObserver never sees that; the re-report is deduped, so this is free
	const tabs = state.data?.tabs;
	// biome-ignore lint/correctness/useExhaustiveDependencies: tabs and overlayOpen are the layout triggers, not values the effect reads
	useEffect(() => remeasure(), [tabs, overlayOpen, remeasure]);

	const active =
		state.data?.tabs.find((t) => t.id === state.data?.active) ?? null;
	// nothing until the first snapshot lands, so the start page never flashes
	// over a tab that is already loading
	const showStart =
		state.data !== undefined && (active === null || active.url === null);

	function onNavigate(url: string) {
		if (active !== null) navigate.mutate({ id: active.id, url });
		else openTab.mutate(url);
	}

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
				onNavigate={onNavigate}
				onGo={(delta) => {
					if (active !== null) go.mutate({ id: active.id, delta });
				}}
			>
				<DownloadsButton
					open={downloadsOpen}
					onToggle={() => setDownloadsOpen((open) => !open)}
				/>
			</Toolbar>
			<div className="flex min-h-0 flex-1">
				<div ref={contentRef} className="min-h-0 flex-1">
					{showStart && <StartPage onNavigate={onNavigate} />}
				</div>
				{downloadsOpen && <DownloadsPanel />}
			</div>
			<NeedsProjectSheet />
		</div>
	);
}
