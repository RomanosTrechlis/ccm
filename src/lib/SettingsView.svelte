<script lang="ts">
	import { app, saveSettings, refreshNow } from '$lib/state.svelte';
	import { num } from '$lib/fmt';

	let budget = $state(0);
	let fast = $state(7);
	let slow = $state(45);
	let saved = $state(false);
	let claudePct = $state(0);
	let calibrated = $state('');

	$effect(() => {
		budget = app.settings.tokenBudget;
		fast = app.settings.blocksPollSecs;
		slow = app.settings.summaryPollSecs;
	});

	async function save() {
		await saveSettings({ tokenBudget: budget, blocksPollSecs: fast, summaryPollSecs: slow });
		saved = true;
		setTimeout(() => (saved = false), 2000);
	}

	const canCalibrate = $derived(
		!!app.gauge?.active && app.gauge.used > 0 && claudePct >= 1 && claudePct <= 100
	);

	// budget so that ccm's % == Claude Code's %, rounded to 100k tokens
	async function calibrate() {
		if (!app.gauge) return;
		const b = Math.max(1_000_000, Math.round(app.gauge.used / (claudePct / 100) / 1e5) * 1e5);
		await saveSettings({ tokenBudget: b, blocksPollSecs: fast, summaryPollSecs: slow });
		calibrated = `Budget set to ${num(b)} tokens — gauge now reads ${Math.round((app.gauge.used / b) * 100)}%.`;
	}
</script>

<div class="col">
	<div class="card">
		<h3>ccusage</h3>
		{#if app.status?.found}
			<p>
				Version <strong>{app.status.version}</strong> via <strong>{app.status.source}</strong>
				<span class="chip">pinned fallback {app.status.pinned}</span>
			</p>
		{:else if app.status}
			<p style="color: var(--red)">Not found — {app.status.error}</p>
		{:else}
			<p style="color: var(--muted)">Detecting…</p>
		{/if}
		<button class="ghost" onclick={refreshNow}>Refresh now</button>
	</div>

	<div class="card">
		<h3>Calibrate against Claude Code <span class="estimate">recommended</span></h3>
		<div class="hint" style="margin-top: 0">
			ccm estimates block usage as <em>tokens ÷ your budget</em>; Claude Code's
			<code>/usage</code> shows Anthropic's real (non-public) limit. They drift because the real
			limit isn't a plain token count. Enter what <code>/usage</code> shows right now and ccm
			sets the budget so both match.
		</div>
		<label for="cpct">Claude Code /usage shows (%)</label>
		<input id="cpct" type="number" min="1" max="100" bind:value={claudePct} />
		<div>
			<button class="primary" disabled={!canCalibrate} onclick={calibrate}>
				Set budget from {claudePct || '…'}%
			</button>
		</div>
		{#if !app.gauge?.active}
			<div class="hint">Needs an active block — use Claude Code for a minute first.</div>
		{:else if calibrated}
			<div class="hint" style="color: var(--green)">{calibrated}</div>
		{/if}
	</div>

	<div class="card">
		<h3>Gauge &amp; polling</h3>

		<label for="budget">5-hour token budget <span class="estimate">estimate</span></label>
		<input id="budget" type="number" min="1" step="1000000" bind:value={budget} />
		<div class="hint">
			= {num(budget)} tokens (includes cache reads — budgets look huge; that's normal). The ring
			gauge and tray color measure against this. Calibration above sets it for you.
		</div>

		<label for="fast">Active-block poll interval (5–10 s)</label>
		<input id="fast" type="number" min="5" max="10" bind:value={fast} />

		<label for="slow">Daily / sessions poll interval (30–60 s)</label>
		<input id="slow" type="number" min="30" max="60" bind:value={slow} />

		<div>
			<button class="primary" onclick={save}>{saved ? 'Saved ✓' : 'Save'}</button>
		</div>
	</div>
</div>

<style>
	.col { display: flex; flex-direction: column; gap: 14px; max-width: 620px; }
	p { margin: 4px 0 12px; }
</style>
