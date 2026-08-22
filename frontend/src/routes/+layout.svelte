<script lang="ts">
	import favicon from '$lib/assets/favicon.svg';
	import { page } from '$app/stores';

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>inferwatch</title>
	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link
		href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap"
		rel="stylesheet"
	/>
</svelte:head>

<nav>
	<a href="/" class="brand">
		<span class="logo">◉</span> inferwatch
	</a>
	<div class="links">
		<a href="/" class:active={$page.url.pathname === '/'}>Chat</a>
		<a href="/conversations" class:active={$page.url.pathname.startsWith('/conversations')}>Conversations</a>
		<a href="/dashboard" class:active={$page.url.pathname.startsWith('/dashboard')}>Dashboard</a>
	</div>
</nav>

<main>
	{@render children()}
</main>

<style>
	:global(*) {
		box-sizing: border-box;
	}
	:global(html, body) {
		margin: 0;
		padding: 0;
	}
	:global(body) {
		font-family:
			'Inter',
			system-ui,
			-apple-system,
			sans-serif;
		background: #0b0e14;
		color: #e6e9f0;
		font-size: 15px;
		line-height: 1.6;
		-webkit-font-smoothing: antialiased;
	}
	/* theme tokens */
	:global(:root) {
		--bg: #0b0e14;
		--panel: #131823;
		--panel-2: #171e2b;
		--border: #1f2733;
		--border-strong: #2a3547;
		--accent: #818cf8;
		--accent-soft: rgba(129, 140, 248, 0.14);
		--text: #e6e9f0;
		--muted: #8b94a7;
		--ok: #62d2a2;
		--err: #f38787;
		--radius: 14px;
	}
	:global(::selection) {
		background: rgba(129, 140, 248, 0.35);
	}
	/* thin unobtrusive scrollbars everywhere */
	:global(::-webkit-scrollbar) {
		width: 6px;
		height: 6px;
	}
	:global(::-webkit-scrollbar-thumb) {
		background: #2a3547;
		border-radius: 3px;
	}
	:global(::-webkit-scrollbar-track) {
		background: transparent;
	}

	nav {
		position: sticky;
		top: 0;
		z-index: 20;
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 0.7rem clamp(1rem, 4vw, 2rem);
		background: rgba(11, 14, 20, 0.8);
		backdrop-filter: blur(12px);
		-webkit-backdrop-filter: blur(12px);
		border-bottom: 1px solid var(--border);
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 0.45rem;
		font-weight: 700;
		font-size: 0.95rem;
		color: var(--text) !important;
		text-decoration: none;
	}
	.brand .logo {
		color: var(--accent);
		font-size: 1.1rem;
	}
	.links {
		display: flex;
		gap: 0.25rem;
		margin-left: 0.5rem;
	}
	.links a {
		color: var(--muted);
		text-decoration: none;
		font-size: 0.88rem;
		font-weight: 500;
		padding: 0.4rem 0.8rem;
		border-radius: 8px;
		transition:
			color 0.15s,
			background 0.15s;
	}
	.links a:hover {
		color: var(--text);
		background: var(--panel-2);
	}
	.links a.active {
		color: var(--text);
		background: var(--accent-soft);
	}
	main {
		max-width: 60rem;
		margin: 0 auto;
		padding: clamp(1rem, 3vw, 2rem);
	}

	@media (max-width: 640px) {
		nav {
			gap: 0.5rem;
		}
		.links a {
			padding: 0.35rem 0.55rem;
			font-size: 0.82rem;
		}
		main {
			padding: 0.9rem;
		}
	}
</style>
