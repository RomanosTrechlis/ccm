<script lang="ts">
	import { app } from '$lib/state.svelte';
	import { num, usd, mins, todayStr, daysAgoStr, dayLabel, modelShort, bandColor } from '$lib/fmt';

	let { onSettings }: { onSettings: () => void } = $props();

	const g = $derived(app.gauge);
	const pct = $derived(Math.min(g?.percent ?? 0, 1));
	const color = $derived(bandColor(g, false));

	const R = 100;
	const C = 2 * Math.PI * R;

	const days = $derived((app.daily?.daily ?? []) as any[]);
	const today = $derived(days.find((d) => d.date === todayStr()));
	const weekStart = $derived(daysAgoStr(6));
	const weekCost = $derived(
		days.filter((d) => d.date >= weekStart).reduce((s, d) => s + (d.totalCost ?? 0), 0)
	);

	// ceiling ETA vs. block end: the block resets at endTime regardless of burn
	const etaLabel = $derived.by(() => {
		if (!g?.active) return '—';
		if (g.percent >= 1) return 'over budget';
		if (g.etaMinutes == null) return 'no burn';
		if (g.blockRemainingMinutes != null && g.etaMinutes > g.blockRemainingMinutes)
			return 'after reset';
		return `~${mins(g.etaMinutes)}`;
	});

	const chart = $derived.by(() => {
		const last = days.slice(-14);
		const max = Math.max(...last.map((d) => d.totalCost ?? 0), 0.01);
		const rows = last.map((d) => {
			const tokens = (d.totalTokens ?? 0) as number;
			const cache = (d.cacheCreationTokens ?? 0) + (d.cacheReadTokens ?? 0);
			return {
				date: d.date as string,
				cost: (d.totalCost ?? 0) as number,
				h: Math.max(3, ((d.totalCost ?? 0) / max) * 100),
				tokens,
				cachePct: tokens > 0 ? Math.round((cache / tokens) * 100) : 0,
				models: [...(d.modelBreakdowns ?? [])]
					.sort((a: any, b: any) => (b.cost ?? 0) - (a.cost ?? 0))
					.slice(0, 3)
					.map((m: any) => ({ name: modelShort(m.modelName ?? '?'), cost: (m.cost ?? 0) as number }))
			};
		});
		const total = rows.reduce((s, r) => s + r.cost, 0);
		const avg = rows.length ? total / rows.length : 0;
		return { rows, total, avg, avgPct: Math.min((avg / max) * 100, 100) };
	});

	let hover = $state<number | null>(null);
</script>

<div class="grid">
	<section class="card gauge-card">
		<h3>5-hour block <span class="estimate">estimate</span></h3>
		<svg viewBox="0 0 232 232" width="216" height="216">
			<circle cx="116" cy="116" r={R} fill="none" stroke="var(--panel2)" stroke-width="17" />
			<circle
				cx="116"
				cy="116"
				r={R}
				fill="none"
				stroke={color}
				stroke-width="17"
				stroke-linecap="round"
				stroke-dasharray={C}
				stroke-dashoffset={C * (1 - pct)}
				transform="rotate(-90 116 116)"
				style="transition: stroke-dashoffset 0.6s ease, stroke 0.6s; filter: drop-shadow(0 0 9px color-mix(in srgb, {color} 55%, transparent))"
			/>
			{#if g?.active}
				<text x="116" y="110" text-anchor="middle" fill="var(--text)" font-size="42" font-weight="650">
					{Math.round((g.percent ?? 0) * 100)}%
				</text>
				<text x="116" y="137" text-anchor="middle" fill="var(--muted)" font-size="12.5">
					{num(g.used)} / {num(g.budget)} tok
				</text>
				{#if g.blockRemainingMinutes != null}
					<text x="116" y="158" text-anchor="middle" fill="var(--muted)" font-size="11">
						resets in {mins(g.blockRemainingMinutes)}
					</text>
				{/if}
			{:else}
				<text x="116" y="121" text-anchor="middle" fill="var(--muted)" font-size="14">no active block</text>
			{/if}
		</svg>
		<button class="linkish" onclick={onSettings}>
			Differs from Claude Code's /usage? Calibrate →
		</button>
	</section>

	<section class="stats">
		<div class="card">
			<h3>Burn rate</h3>
			<div class="big">{g?.active ? num(g.burnPerMin) : '—'} <small>tok/min</small></div>
			<div class="sub">{g?.active ? usd(g.costPerHour) + ' / hour' : 'waiting for an active block'}</div>
		</div>
		<div class="card">
			<h3>Ceiling ETA</h3>
			<div class="big">{etaLabel}</div>
			<div class="sub">
				{#if g?.active && g.blockRemainingMinutes != null}
					at current burn · block resets in {mins(g.blockRemainingMinutes)}
				{:else}
					at current burn rate
				{/if}
			</div>
		</div>
		<div class="card">
			<h3>Block cost</h3>
			<div class="big">{g?.active ? usd(g.blockCost) : '—'}</div>
			<div class="sub">
				{#if app.block?.models?.length}
					{app.block.models.map(modelShort).join(' · ')}
				{:else}
					this 5-hour block
				{/if}
			</div>
		</div>
		<div class="card">
			<h3>Today</h3>
			<div class="big">{usd(today?.totalCost)}</div>
			<div class="sub">{today ? num(today.totalTokens) + ' tokens' : 'no usage yet today'}</div>
		</div>
		<div class="card">
			<h3>Last 7 days</h3>
			<div class="big">{usd(weekCost)}</div>
			<div class="sub">{days.length ? 'including today' : 'no data yet'}</div>
		</div>
	</section>

	{#if chart.rows.length > 1}
		<section class="card chart-card">
			<div class="chart-head">
				<h3>Daily cost · last {chart.rows.length} days</h3>
				<span class="totals">total {usd(chart.total)} · avg {usd(chart.avg)}/day</span>
			</div>
			<div class="bars">
				<div class="avg" style="bottom: {chart.avgPct}%"><span>avg {usd(chart.avg)}</span></div>
				{#each chart.rows as d, i (d.date)}
					<div
						class="col"
						role="img"
						aria-label="{dayLabel(d.date)}: {usd(d.cost)}"
						onmouseenter={() => (hover = i)}
						onmouseleave={() => (hover = null)}
					>
						<div class="b" class:today={d.date === todayStr()} style="height: {d.h}%"></div>
						{#if hover === i}
							<div class="tip" class:tleft={i < 3} class:tright={i > chart.rows.length - 4}>
								<div class="tip-date">{dayLabel(d.date)}</div>
								<div class="tip-main">{usd(d.cost)} · {num(d.tokens)} tok · {d.cachePct}% cache</div>
								{#each d.models as m (m.name)}
									<div class="tip-model"><span>{m.name}</span><span>{usd(m.cost)}</span></div>
								{/each}
							</div>
						{/if}
					</div>
				{/each}
			</div>
			<div class="xlabels">
				{#each chart.rows as d (d.date)}
					<span class:today={d.date === todayStr()}>{+d.date.slice(8)}</span>
				{/each}
			</div>
		</section>
	{/if}
</div>

{#if !days.length && !g?.active && !app.error}
	<div class="empty" style="margin-top: 16px">
		No Claude Code usage data yet.<br />Data appears here after your first Claude Code session.
	</div>
{/if}

<style>
	.grid { display: grid; grid-template-columns: 300px 1fr; gap: 14px; }
	.gauge-card { text-align: center; }
	.stats {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
		gap: 14px;
		align-content: start;
	}
	.chart-card { grid-column: 1 / -1; }
	.chart-head { display: flex; align-items: baseline; justify-content: space-between; }
	.totals { color: var(--muted); font-size: 12px; font-variant-numeric: tabular-nums; }

	.bars {
		position: relative;
		display: flex;
		gap: 4px;
		height: 96px;
		align-items: flex-end;
		margin-top: 10px;
	}
	.avg {
		position: absolute;
		left: 0;
		right: 0;
		border-top: 1px dashed color-mix(in srgb, var(--amber) 55%, transparent);
		pointer-events: none;
	}
	.avg span {
		position: absolute;
		right: 0;
		bottom: 2px;
		font-size: 10px;
		color: var(--amber);
		opacity: 0.85;
	}
	.col { flex: 1; height: 100%; display: flex; align-items: flex-end; position: relative; }
	.b {
		width: 100%;
		background: linear-gradient(180deg, var(--accent), var(--accent-deep));
		border-radius: 4px 4px 2px 2px;
		min-height: 3px;
		opacity: 0.65;
		transition: opacity 0.15s, height 0.4s ease;
	}
	.col:hover .b { opacity: 1; }
	.b.today { background: linear-gradient(180deg, var(--green), #22995c); opacity: 1; }

	.xlabels { display: flex; gap: 4px; margin-top: 6px; }
	.xlabels span {
		flex: 1;
		text-align: center;
		font-size: 10px;
		color: var(--muted);
		font-variant-numeric: tabular-nums;
	}
	.xlabels span.today { color: var(--green); font-weight: 600; }

	.tip {
		position: absolute;
		bottom: calc(100% + 8px);
		left: 50%;
		transform: translateX(-50%);
		background: var(--panel3);
		border: 1px solid var(--border-hi);
		border-radius: 10px;
		padding: 9px 12px;
		box-shadow: 0 6px 24px rgba(0, 0, 0, 0.5);
		z-index: 10;
		white-space: nowrap;
		pointer-events: none;
		font-size: 12px;
		text-align: left;
	}
	.tip.tleft { left: 0; transform: none; }
	.tip.tright { left: auto; right: 0; transform: none; }
	.tip-date { font-weight: 600; margin-bottom: 2px; }
	.tip-main { color: var(--muted); margin-bottom: 5px; }
	.tip-model {
		display: flex;
		justify-content: space-between;
		gap: 16px;
		color: var(--muted);
		font-variant-numeric: tabular-nums;
	}
	.tip-model span:first-child { color: var(--text); }
	.linkish {
		background: none;
		border: none;
		color: var(--muted);
		font-size: 11.5px;
		cursor: pointer;
		padding: 4px 0 0;
	}
	.linkish:hover { color: var(--accent); }
	@media (max-width: 780px) {
		.grid { grid-template-columns: 1fr; }
	}
</style>
