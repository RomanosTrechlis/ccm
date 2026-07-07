<script lang="ts">
	import { app } from '$lib/state.svelte';
	import { num, usd, timeAgo, modelShort } from '$lib/fmt';

	/** "C--Users-Anubis-workspaces-ccm" -> "ccm" (full path in tooltip) */
	function projectName(p: string | undefined): string {
		if (!p) return '—';
		return p.split('-').filter(Boolean).pop() ?? p;
	}

	const rows = $derived.by(() => {
		const list = [...((app.sessions?.sessions ?? []) as any[])];
		list.sort((a, b) => (b.lastActivity ?? '').localeCompare(a.lastActivity ?? ''));
		return list.slice(0, 100);
	});
</script>

{#if rows.length}
	<div class="card table-card">
		<table>
			<thead>
				<tr>
					<th>Project</th>
					<th>Session</th>
					<th>Last activity</th>
					<th>Models</th>
					<th class="num">Tokens</th>
					<th class="num">Cost</th>
				</tr>
			</thead>
			<tbody>
				{#each rows as s (s.sessionId)}
					<tr>
						<td title={s.projectPath}><strong>{projectName(s.projectPath)}</strong></td>
						<td title={s.sessionId}><code>{(s.sessionId ?? '').slice(0, 8)}</code></td>
						<td>{timeAgo(s.lastActivity)}</td>
						<td>
							{#each s.modelsUsed ?? [] as m (m)}
								<span class="chip">{modelShort(m)}</span>
							{/each}
						</td>
						<td class="num">{num(s.totalTokens)}</td>
						<td class="num">{usd(s.totalCost)}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
{:else}
	<div class="empty">No sessions yet — start a Claude Code session and refresh.</div>
{/if}
