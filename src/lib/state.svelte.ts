import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type Gauge = {
	active: boolean;
	used: number;
	budget: number;
	percent: number;
	burnPerMin: number;
	costPerHour: number;
	etaMinutes: number | null;
	blockCost: number;
	blockEnd: string | null;
	blockRemainingMinutes: number | null;
	costToday: number;
};

export type Settings = {
	tokenBudget: number;
	blocksPollSecs: number;
	summaryPollSecs: number;
};

export type CcusageStatus = {
	found: boolean;
	version: string | null;
	source: string | null;
	pinned: string;
	error: string | null;
};

// mirrors the Rust cache; events keep it live, get_snapshot seeds it on mount
export const app = $state({
	gauge: null as Gauge | null,
	block: null as any, // raw ccusage active-block JSON (tokenCounts, models, projection)
	daily: null as any, // raw `ccusage claude daily --json` report
	sessions: null as any, // raw `ccusage claude session --json` report
	error: null as string | null,
	status: null as CcusageStatus | null,
	settings: { tokenBudget: 50_000_000, blocksPollSecs: 7, summaryPollSecs: 45 } as Settings
});

let started = false;

export async function init() {
	if (started) return;
	started = true;

	await Promise.all([
		listen<any>('ccm://active-block', (e) => {
			app.gauge = e.payload.gauge;
			app.block = e.payload.block;
			app.error = null;
		}),
		listen<any>('ccm://usage', (e) => {
			app.daily = e.payload.daily;
			app.sessions = e.payload.sessions;
			app.error = null;
		}),
		listen<any>('ccm://error', (e) => {
			app.error = e.payload.message;
		})
	]);

	const snap: any = await invoke('get_snapshot');
	app.gauge ??= snap.gauge;
	app.block ??= snap.block;
	app.daily ??= snap.daily;
	app.sessions ??= snap.sessions;
	if (snap.error) app.error = snap.error;

	app.settings = await invoke<Settings>('get_settings');
	// may run detection (first npx download can take a while) — don't block the UI on it
	invoke<CcusageStatus>('ccusage_status').then(
		(s) => (app.status = s),
		() => {}
	);
}

export async function saveSettings(s: Settings) {
	app.settings = await invoke<Settings>('set_settings', { settings: s });
}

export function refreshNow() {
	invoke('refresh_now').catch(() => {});
}
