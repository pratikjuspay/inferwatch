import { writable } from 'svelte/store';
import { browser } from '$app/environment';

/// Browser identity — no auth, just a random UUID in localStorage.
/// Conversations are scoped to it. Tradeoff documented in README.
function createSessionStore() {
	const key = 'inferwatch_session_id';
	const initial =
		browser && localStorage.getItem(key)
			? localStorage.getItem(key)!
			: crypto.randomUUID();

	if (browser) localStorage.setItem(key, initial);

	return writable<string>(initial);
}

export const sessionId = createSessionStore();
