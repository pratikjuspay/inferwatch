<script lang="ts">
	import { onMount } from 'svelte';
	import { marked } from 'marked';
	import DOMPurify from 'dompurify';
	import { sessionId } from '$lib/stores';
	import {
		createConversation,
		getConversation,
		listConversations,
		streamChat,
		type ChatMessage
	} from '$lib/api';

	let conversationId: string | null = $state(null);
	let messages = $state<ChatMessage[]>([]);
	let input = $state('');
	let sending = $state(false);
	let error = $state('');
	let chatEl: HTMLDivElement | undefined = $state(undefined);
	let taEl: HTMLTextAreaElement | undefined = $state(undefined);
	let expanded = $state(false);

	const AUTO_MAX = 200; // ~8 lines before inner scroll kicks in

	/// ChatGPT-style auto-grow: hug content up to AUTO_MAX, then scroll inside.
	function autoResize() {
		if (!taEl || expanded) return;
		taEl.style.height = 'auto';
		taEl.style.height = Math.min(taEl.scrollHeight, AUTO_MAX) + 'px';
	}

	function resetSize() {
		if (!taEl) return;
		taEl.style.height = '';
		expanded = false;
	}

	function md(text: string): string {
		const html = marked.parse(text, { async: false, gfm: true }) as string;
		return DOMPurify.sanitize(html);
	}

	// scroll to bottom whenever any message content changes (each token)
	$effect(() => {
		for (const m of messages) void m.content;
		chatEl?.scrollTo({ top: chatEl.scrollHeight, behavior: 'smooth' });
	});

	// resume: ?c=<id> when coming from the list, else most recent
	onMount(async () => {
		const wanted = new URLSearchParams(location.search).get('c');
		const convos = await listConversations($sessionId);
		const target = wanted ?? convos[0]?.id;
		if (target && convos.some((c) => c.id === target)) {
			const detail = await getConversation(target, $sessionId);
			conversationId = detail.id;
			messages = detail.messages.filter((m) => !(m.role === 'assistant' && !m.content));
		}
	});

	async function ensureConversation(): Promise<string> {
		if (conversationId) return conversationId;
		const c = await createConversation($sessionId);
		conversationId = c.id;
		return c.id;
	}

	async function send() {
		const text = input.trim();
		if (!text || sending) return;
		error = '';
		sending = true;
		resetSize(); // collapse editor immediately on send

		try {
			const id = await ensureConversation();

			messages = [
				...messages,
				{
					id: crypto.randomUUID(),
					role: 'user',
					content: text,
					created_at: new Date().toISOString()
				}
			];
			input = '';

			// index, not captured object: $state proxies array elements on push,
			// so mutating a captured plain object bypasses reactivity.
			const assistantIndex = messages.length;
			messages = [
				...messages,
				{ id: crypto.randomUUID(), role: 'assistant', content: '', created_at: new Date().toISOString() }
			];

			await streamChat(id, $sessionId, text, (e) => {
				if (e.type === 'token') {
					messages[assistantIndex].content += e.content;
				} else if (e.type === 'error') {
					error = e.message;
				}
			});
		} catch (err) {
			error = String(err);
		} finally {
			sending = false;
		}
	}
</script>

<div class="chat" bind:this={chatEl} aria-live="polite">
	{#if messages.length === 0}
		<div class="empty">
			<div class="empty-logo">◉</div>
			<h1>inferwatch</h1>
			<p>Ask anything. Every inference under the hood is timed, token-counted and logged.<br />Check the <a href="/dashboard">dashboard</a> after a few messages.</p>
		</div>
	{/if}

	{#each messages as m (m.id)}
		{#if m.role === 'user'}
			<div class="row user">
				<div class="bubble user-bubble">{m.content}</div>
			</div>
		{:else}
			<div class="row assistant">
				<div class="answer">
					{#if m.content}
						{@html md(m.content)}
					{:else if sending && m === messages[messages.length - 1]}
						<span class="typing"><i></i><i></i><i></i> model is thinking…</span>
					{/if}
				</div>
			</div>
		{/if}
	{/each}

	{#if error}
		<div class="error-box">⚠ {error}</div>
	{/if}
</div>

<form class="composer" class:expanded onsubmit={(e) => { e.preventDefault(); send(); }}>
	<textarea
		bind:this={taEl}
		bind:value={input}
		placeholder="Ask anything…  (Enter to send, Shift+Enter for a new line)"
		rows="1"
		oninput={autoResize}
		onkeydown={(e) => {
			if (e.key === 'Enter' && !e.shiftKey) {
				e.preventDefault();
				send();
			}
		}}
	></textarea>
	<button
		type="button"
		class="expand-btn"
		title={expanded ? 'Collapse editor' : 'Expand editor'}
		onclick={() => {
			expanded = !expanded;
			// inline height (from auto-grow) overrides the .expanded CSS —
			// clear it on the way in, recompute on the way out
			if (expanded) {
				if (taEl) taEl.style.height = '';
			} else {
				autoResize();
			}
		}}
	>
		{expanded ? '⤡' : '⤢'}
	</button>
	<button class="send" type="submit" disabled={sending || !input.trim()} aria-label="send">
		{#if sending}
			<span class="spinner"></span>
		{:else}
			↑
		{/if}
	</button>
</form>

<style>
	.chat {
		display: flex;
		flex-direction: column;
		gap: 1.1rem;
		height: calc(100dvh - 200px);
		min-height: 300px;
		overflow-y: auto;
		padding: 0.25rem 0.25rem 1.5rem;
		scroll-behavior: smooth;
	}

	.empty {
		margin: auto;
		text-align: center;
		color: var(--muted);
		padding: 2rem 1rem;
	}
	.empty-logo {
		font-size: 2rem;
		color: var(--accent);
	}
	.empty h1 {
		font-size: 1.3rem;
		margin: 0.4rem 0;
		color: var(--text);
	}
	.empty p {
		font-size: 0.9rem;
		max-width: 30rem;
		margin: 0 auto;
	}
	.empty a {
		color: var(--accent);
	}

	.row.user {
		display: flex;
		justify-content: flex-end;
	}
	.user-bubble {
		background: var(--accent-soft);
		border: 1px solid rgba(129, 140, 248, 0.25);
		color: var(--text);
		padding: 0.55rem 1rem;
		border-radius: 18px 18px 4px 18px;
		max-width: min(75%, 46rem);
		white-space: pre-wrap;
		font-size: 0.94rem;
	}

	/* assistant: flat, document-style — no bubble, markdown becomes layout */
	.answer {
		max-width: 100%;
		font-size: 0.97rem;
		line-height: 1.7;
	}
	.answer :global(h1),
	.answer :global(h2),
	.answer :global(h3),
	.answer :global(h4) {
		margin: 1.1em 0 0.4em;
		line-height: 1.3;
		color: var(--text);
	}
	.answer :global(h3) {
		font-size: 1.06rem;
	}
	.answer :global(p) {
		margin: 0.55em 0;
	}
	.answer :global(ul),
	.answer :global(ol) {
		padding-left: 1.3rem;
		margin: 0.5em 0;
	}
	.answer :global(li) {
		margin: 0.3em 0;
	}
	.answer :global(code) {
		background: var(--panel-2);
		border: 1px solid var(--border);
		border-radius: 6px;
		padding: 0.1em 0.4em;
		font-size: 0.85em;
	}
	.answer :global(pre) {
		background: var(--panel);
		border: 1px solid var(--border);
		border-radius: 10px;
		padding: 0.9rem 1rem;
		overflow-x: auto;
	}
	.answer :global(pre code) {
		background: none;
		border: none;
		padding: 0;
	}
	.answer :global(strong) {
		color: var(--text);
		font-weight: 600;
	}
	.answer :global(a) {
		color: var(--accent);
	}

	.typing {
		color: var(--muted);
		font-style: italic;
		display: inline-flex;
		align-items: baseline;
		gap: 4px;
	}
	.typing i {
		display: inline-block;
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--accent);
		animation: bounce 1.2s infinite;
	}
	.typing i:nth-child(2) { animation-delay: 0.15s; }
	.typing i:nth-child(3) { animation-delay: 0.3s; }
	@keyframes bounce {
		0%, 60%, 100% { transform: translateY(0); opacity: 0.4; }
		30% { transform: translateY(-4px); opacity: 1; }
	}

	.error-box {
		background: rgba(243, 135, 135, 0.08);
		border: 1px solid rgba(243, 135, 135, 0.3);
		color: var(--err);
		border-radius: 10px;
		padding: 0.7rem 1rem;
		font-size: 0.88rem;
	}

	.composer {
		position: sticky; /* sticky also anchors the absolute expand-btn */
		bottom: 0;
		display: flex;
		align-items: flex-end;
		gap: 0.6rem;
		padding: 0.6rem 0.25rem 0;
		background: linear-gradient(transparent, var(--bg) 30%);
	}
	textarea {
		flex: 1;
		background: var(--panel);
		border: 1px solid var(--border-strong);
		border-radius: 22px;
		color: var(--text);
		padding: 0.85rem 2.4rem 0.85rem 1.2rem; /* right space for expand-btn */
		font: inherit;
		font-size: 0.95rem;
		resize: none;
		min-height: 46px;
		max-height: 200px;
		overflow-y: auto;
		transition: border-color 0.15s, box-shadow 0.15s, border-radius 0.15s, max-height 0.15s;
	}
	textarea:focus {
		outline: none;
		border-color: var(--accent);
		box-shadow: 0 0 0 3px var(--accent-soft);
	}

	/* expanded = big editor panel, like GPT's expand mode */
	.composer.expanded textarea {
		border-radius: 14px 14px 0 0;
		max-height: min(55vh, 600px);
		height: min(55vh, 600px);
		overflow-y: auto;
		border-bottom: none;
	}
	.expand-btn {
		position: absolute;
		right: 76px;
		top: 50%;
		transform: translateY(-50%);
		width: 30px;
		height: 30px;
		border-radius: 8px;
		border: 1px solid var(--border);
		background: var(--panel-2);
		color: var(--muted);
		font-size: 0.95rem;
		cursor: pointer;
		display: grid;
		place-items: center;
		transition: color 0.15s, border-color 0.15s;
	}
	.expand-btn:hover {
		color: var(--text);
		border-color: var(--border-strong);
	}
	.composer.expanded .expand-btn {
		top: 14px;
		transform: none;
		border-radius: 0 8px 0 0;
	}
	.send {
		width: 46px;
		height: 46px;
		flex-shrink: 0;
		border-radius: 50%;
		border: none;
		background: var(--accent);
		color: #fff;
		font-size: 1.15rem;
		font-weight: 600;
		cursor: pointer;
		display: grid;
		place-items: center;
		transition: transform 0.12s, opacity 0.15s;
	}
	.send:hover:not(:disabled) {
		transform: translateY(-1px);
	}
	.send:disabled {
		opacity: 0.4;
		cursor: default;
	}
	.spinner {
		width: 18px;
		height: 18px;
		border: 2px solid rgba(255, 255, 255, 0.35);
		border-top-color: #fff;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}
	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	@media (max-width: 640px) {
		.chat {
			height: calc(100dvh - 178px);
		}
		.user-bubble {
			max-width: 88%;
		}
	}
</style>
