<script lang="ts">
	import { onMount } from 'svelte';
	import { app, init, refreshNow } from '$lib/state.svelte';
	import { bandColor } from '$lib/fmt';
	import Overview from '$lib/Overview.svelte';
	import Sessions from '$lib/Sessions.svelte';
	import Breakdown from '$lib/Breakdown.svelte';
	import SettingsView from '$lib/SettingsView.svelte';

	const tabs = [
		['overview', 'Overview'],
		['sessions', 'Sessions'],
		['breakdown', 'Breakdown'],
		['settings', 'Settings']
	] as const;

	let tab = $state<(typeof tabs)[number][0]>('overview');

	const dot = $derived(bandColor(app.gauge, !!app.error));

	onMount(init);
</script>

<main>
	<header>
		<div class="brand">
			<span class="dot" style="background: {dot}; color: {dot}"></span>
			ccm <small>· Claude Code usage</small>
		</div>
		<div class="spacer"></div>
		<nav>
			{#each tabs as [id, label] (id)}
				<button class:active={tab === id} onclick={() => (tab = id)}>{label}</button>
			{/each}
		</nav>
		<button class="ghost" title="Refresh now" aria-label="Refresh now" onclick={refreshNow}>⟳</button>
	</header>

	{#if app.error}
		<div class="banner">
			{#if app.error.includes('not found')}
				<strong>ccusage not found.</strong> ccm reads all data from the
				<code>ccusage</code> CLI. Install Node.js, then run
				<code>npm i -g ccusage</code> — or make sure <code>npx</code> works (ccm falls back to a
				pinned <code>npx ccusage</code>).
			{:else}
				<strong>ccusage error:</strong> {app.error}
			{/if}
		</div>
	{/if}

	{#if tab === 'overview'}
		<Overview onSettings={() => (tab = 'settings')} />
	{:else if tab === 'sessions'}
		<Sessions />
	{:else if tab === 'breakdown'}
		<Breakdown />
	{:else}
		<SettingsView />
	{/if}
</main>
