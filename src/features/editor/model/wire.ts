import type {
	EditHistory,
	EditSnapshot,
	Edits_Deserialize,
	Rational,
} from "@/lib/ipc/bindings.gen";

/** Seconds-domain time range (UI). */
export type Range = { start: number; end: number };
export type ManualCutSec = Range & { note: string | null };
export type BoundaryAdjustmentSec = {
	cutIndex: number;
	edge: "start" | "end";
	newTime: number;
};

/** The live editor document — what zundo history tracks. */
export type EditorDocument = {
	toggledOff: number[];
	appliedDiscretionary: number[];
	manualCuts: ManualCutSec[];
	boundaryAdjustments: BoundaryAdjustmentSec[];
};

export type EditorHistory = {
	past: EditorDocument[];
	future: EditorDocument[];
};

/** Persisted undo depth (PRD-locked). */
export const HISTORY_LIMIT = 100;

/**
 * Frame-exact seconds -> Rational in the video frame timebase (den = fps.num).
 * Frame index k = round(sec * fps.num / fps.den); the tick value is k * fps.den
 * in a den of fps.num — exactly the engine's frame grid.
 */
export function secondsToRational(sec: number, fps: Rational): Rational {
	const k = Math.round((sec * fps.num) / fps.den);
	return { num: k * fps.den, den: fps.num };
}

/** Display projection; boundary use only. */
export function rationalToSeconds(r: Rational): number {
	return r.num / r.den;
}

/**
 * Precision-preserving seconds -> Rational for persisted edit times. Edit
 * times are "any timebase; rescaled to the plan's on merge" (engine contract),
 * and frame-snapping here would corrupt token-snapped ranges — a microsecond
 * grid round-trips every UI value exactly.
 */
const PRECISE_TIMEBASE = 1_000_000;
function secondsToPrecise(sec: number): Rational {
	return { num: Math.round(sec * PRECISE_TIMEBASE), den: PRECISE_TIMEBASE };
}

function toWireSnapshot(doc: EditorDocument): EditSnapshot {
	return {
		toggled_off: doc.toggledOff,
		applied_discretionary: doc.appliedDiscretionary,
		manual_cuts: doc.manualCuts.map((m) => ({
			start: secondsToPrecise(m.start),
			end: secondsToPrecise(m.end),
			note: m.note,
		})),
		boundary_adjustments: doc.boundaryAdjustments.map((b) => ({
			cut_index: b.cutIndex,
			edge: b.edge,
			new_time: secondsToPrecise(b.newTime),
		})),
	};
}

function fromWireSnapshot(snap: EditSnapshot): EditorDocument {
	return {
		toggledOff: snap.toggled_off ?? [],
		appliedDiscretionary: snap.applied_discretionary ?? [],
		manualCuts: (snap.manual_cuts ?? []).map((m) => ({
			start: rationalToSeconds(m.start),
			end: rationalToSeconds(m.end),
			note: m.note,
		})),
		boundaryAdjustments: (snap.boundary_adjustments ?? []).map((b) => ({
			cutIndex: b.cut_index,
			edge: b.edge,
			newTime: rationalToSeconds(b.new_time),
		})),
	};
}

/** Document + history -> the edits.json wire shape, history depth-capped at 100. */
export function toWireEdits(
	doc: EditorDocument,
	history: EditorHistory,
	_fps: Rational,
): Edits_Deserialize {
	const wireHistory: EditHistory = {
		past: history.past.slice(-HISTORY_LIMIT).map(toWireSnapshot),
		future: history.future
			.slice(0, HISTORY_LIMIT)
			.map(toWireSnapshot),
	};
	return {
		schema_version: 1,
		...toWireSnapshot(doc),
		history: wireHistory,
	};
}

/** edits.json (or nothing) -> live document + seeded history. */
export function fromWireEdits(
	edits: Edits_Deserialize | null,
	_fps: Rational,
): { document: EditorDocument; history: EditorHistory } {
	if (edits === null) {
		return {
			document: fromWireSnapshot({}),
			history: { past: [], future: [] },
		};
	}
	return {
		document: fromWireSnapshot(edits),
		history: {
			past: (edits.history?.past ?? []).map(fromWireSnapshot),
			future: (edits.history?.future ?? []).map(fromWireSnapshot),
		},
	};
}
