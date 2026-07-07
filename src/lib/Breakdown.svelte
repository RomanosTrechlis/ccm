<script lang="ts">
	import { app } from '$lib/state.svelte';
	import { num, usd, modelShort, todayStr } from '$lib/fmt';

	type Agg = { label: string; cost: number; tokens: number };

	function aggregate(items: any[], key: (x: any) => string, cost: (x: any) => number, tokens: (x: any) => number): Agg[] {
		const map = new Map<string, Agg>();
		for (const it of items) {
			const k = key(it) || '—';
			const a = map.get(k) ?? { label: k, cost: 0, tokens: 0 };
			a.cost += cost(it) ?? 0;
			a.tokens += tokens(it) ?? 0;
			map.set(k, a);
		}
		return [...map.values()].sort((a, b) => b.cost - a.cost);
	}

	const days = $derived((app.daily?.daily ?? []) as any[]);

	const byModel = $derived(
		aggregate(
			days.flatMap((d) => d.modelBreakdowns ?? []),
			(m) => modelShort(m.modelName ?? '?'),
			(m) => m.cost,
			(m) => (m.inputTokens ?? 0) + (m.outputTokens ?? 0) + (m.cacheCreationTokens ?? 0) + (m.cacheReadTokens ?? 0)
		)
	);

	const byProject = $derived(
		aggregate(
			(app.sessions?.sessions ?? []) as any[],
			(s) => (s.projectPath ?? '—').split('-').filter(Boolean).pop(),
			(s) => s.totalCost,
			(s) => s.totalTokens
		).slice(0, 15)
	);

	// cache vs fresh: fresh = input + output; cache = cacheCreation + cacheRead
	function split(rows: any[]) {
		const sum = (k: string) => rows.reduce((s, r) => s + (r[k] ?? 0), 0);
		const fresh = sum('inputTokens') + sum('outputTokens');
		const cache = sum('cacheCreationTokens') + sum('cacheReadTokens');
		return { fresh, cache, total: fresh + cache };
	}
	const splitToday = $derived(split(days.filter((d) => d.date === todayStr())));
	const splitAll = $derived(split(days));
</script>

{#if days.length}
	<div class="col">
		<div class="card">
			<h3>By model</h3>
			{#each byModel as m (m.label)}
				<div class="bar-row">
					<span class="label" title={m.label}>{m.label}</span>
					<div class="bar-track">
						<div class="bar-fill" style="width: {(m.cost / (byModel[0]?.cost || 1)) * 100}%"></div>
					</div>
					<span class="num">{num(m.tokens)}</span>
					<span class="num">{usd(m.cost)}</span>
				</div>
			{/each}
		</div>

		<div class="card">
			<h3>By project</h3>
			{#if byProject.length}
				{#each byProject as p (p.label)}
					<div class="bar-row">
						<span class="label" title={p.label}>{p.label}</span>
						<div class="bar-track">
							<div class="bar-fill" style="width: {(p.cost / (byProject[0]?.cost || 1)) * 100}%"></div>
						</div>
						<span class="num">{num(p.tokens)}</span>
						<span class="num">{usd(p.cost)}</span>
					</div>
				{/each}
			{:else}
				<div class="sub">No session data yet.</div>
			{/if}
		</div>

		<div class="card">
			<h3>Cache vs fresh tokens</h3>
			{#each [{ label: 'Today', s: splitToday }, { label: 'All history', s: splitAll }] as row (row.label)}
				{#if row.s.total > 0}
					<div class="bar-row">
						<span class="label">{row.label}</span>
						<div class="bar-track split">
							<div class="bar-fill cache" style="width: {(row.s.cache / row.s.total) * 100}%"></div>
						</div>
						<span class="num">{Math.round((row.s.cache / row.s.total) * 100)}% cache</span>
						<span class="num">{num(row.s.fresh)} fresh</span>
					</div>
				{/if}
			{/each}
			<p class="note">
				<span class="legend"><i style="background: var(--accent)"></i>cache (reads + writes, cheap)</span>
				<span class="legend"><i style="background: var(--green)"></i>fresh (uncached input + output — full price)</span>
			</p>
		</div>
	</div>
{:else}
	<div class="empty">No usage data yet — nothing to break down.</div>
{/if}

<style>
	.col { display: flex; flex-direction: column; gap: 14px; }
	.bar-track.split { background: var(--green); }
	.bar-fill.cache { background: linear-gradient(90deg, var(--accent-deep), var(--accent)); }
	.note { margin: 10px 0 4px; }
</style>
