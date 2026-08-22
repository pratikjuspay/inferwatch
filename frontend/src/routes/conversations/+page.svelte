<script lang="ts">
	import { onMount } from 'svelte';
	import { sessionId } from '$lib/stores';
	import { listConversations, type Conversation } from '$lib/api';

	let conversations = $state<Conversation[]>([]);
	let loading = $state(true);

	onMount(async () => {
		try {
			conversations = await listConversations($sessionId);
		} finally {
			loading = false;
		}
	});

	function rel(date: string): string {
		const m = Math.floor((Date.now() - new Date(date).getTime()) / 60000);
		if (m < 1) return 'just now';
		if (m < 60) return `${m}m ago`;
		const h = Math.floor(m / 60);
		if (h < 24) return `${h}h ago`;
		const d = Math.floor(h / 24);
		if (d < 7) return `${d}d ago`;
		return new Date(date).toLocaleDateString();
	}
</script>

<div class="head">
	<h1>Conversations</h1>
	<a class="new-btn" href="/">+ New chat</a>
</div>

{#if loading}
	<p class="muted">loading…</p>
{:else if conversations.length === 0}
	<div class="none">
		<p>No conversations yet.</p>
		<p class="muted">Every chat is timestamped, logged and persists across refreshes.</p>
	</div>
{:else}
	<div class="grid">
		{#each conversations as c, i (c.id)}
			<a href="/?c={c.id}" class="card">
				<span class="title">{c.title || 'Untitled'}</span>
				<span class="date">{rel(c.updated_at)}</span>
			</a>
		{/each}
	</div>
{/if}

<style>
	.head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 1.25rem;
	}
	h1 {
		font-size: 1.35rem;
		margin: 0;
	}
	.new-btn {
		background: var(--accent);
		color: #fff;
		font-size: 0.85rem;
		font-weight: 600;
		padding: 0.5rem 1.1rem;
		border-radius: 22px;
		text-decoration: none;
		transition: opacity 0.15s;
	}
	.new-btn:hover { opacity: 0.85; }

	.grid {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
	}
	.card {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.9rem 1.1rem;
		background: var(--panel);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		text-decoration: none;
		color: inherit;
		transition: border-color 0.15s, background 0.15s;
	}
	.card:hover {
		border-color: var(--accent);
		background: var(--panel-2);
	}
	.title {
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.date {
		color: var(--muted);
		font-size: 0.78rem;
		white-space: nowrap;
	}
	.none {
		text-align: center;
		padding: 4rem 1rem;
	}
	.muted { color: var(--muted); font-size: 0.9rem; }
</style>
