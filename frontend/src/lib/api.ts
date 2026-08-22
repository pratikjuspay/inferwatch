const API = 'http://localhost:3001';

export interface Conversation {
	id: string;
	title: string;
	created_at: string;
	updated_at: string;
}

export interface ChatMessage {
	id: string;
	role: 'user' | 'assistant' | 'system';
	content: string;
	created_at: string;
}

export interface ConversationDetail extends Conversation {
	messages: ChatMessage[];
}

export interface MetricsSummary {
	total_calls: number;
	error_count: number;
	error_rate: number;
	avg_latency_ms: number;
	p95_latency_ms: number | null;
	total_input_tokens: number;
	total_output_tokens: number;
	calls_last_hour: number;
}

export interface LatencyPoint {
	bucket: string;
	avg_latency_ms: number;
	calls: number;
	error_count: number;
}

export interface LogRow {
	id: string;
	conversation_id: string;
	model: string;
	provider: string;
	latency_ms: number;
	input_tokens: number | null;
	output_tokens: number | null;
	status: 'success' | 'error';
	error_msg: string | null;
	input_preview: string | null;
	output_preview: string | null;
	created_at: string;
}

export async function createConversation(sessionId: string): Promise<Conversation> {
	const res = await fetch(`${API}/api/conversations`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ session_id: sessionId })
	});
	if (!res.ok) throw new Error(await res.text());
	return res.json();
}

export async function listConversations(sessionId: string): Promise<Conversation[]> {
	const res = await fetch(`${API}/api/conversations?session_id=${sessionId}`);
	if (!res.ok) throw new Error(await res.text());
	return res.json();
}

export async function getConversation(id: string, sessionId: string): Promise<ConversationDetail> {
	const res = await fetch(`${API}/api/conversations/${id}?session_id=${sessionId}`);
	if (!res.ok) throw new Error(await res.text());
	return res.json();
}

export type ChatStreamEvent =
	| { type: 'token'; content: string }
	| { type: 'done'; message_id: string; input_tokens: number | null; output_tokens: number | null }
	| { type: 'error'; message: string };

/// POST /chat/:id and consume the SSE stream, invoking onEvent per chunk.
/// SSE over POST — EventSource is GET-only, so we parse the stream manually.
export async function streamChat(
	conversationId: string,
	sessionId: string,
	message: string,
	onEvent: (e: ChatStreamEvent) => void,
	signal?: AbortSignal
): Promise<void> {
	const res = await fetch(`${API}/api/chat/${conversationId}`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ session_id: sessionId, message }),
		signal
	});
	if (!res.ok || !res.body) throw new Error(await res.text());

	const reader = res.body.getReader();
	const decoder = new TextDecoder();
	let buffer = '';

	while (true) {
		const { value, done } = await reader.read();
		if (done) break;
		buffer += decoder.decode(value, { stream: true });

		// SSE frames are separated by blank lines
		while (buffer.includes('\n\n')) {
			const idx = buffer.indexOf('\n\n');
			const frame = buffer.slice(0, idx);
			buffer = buffer.slice(idx + 2);

			for (const line of frame.split('\n')) {
				if (line.startsWith('data: ')) {
					const data = line.slice(6);
					if (data === '[DONE]') return;
					onEvent(JSON.parse(data) as ChatStreamEvent);
				}
			}
		}
	}
}

export async function fetchMetricsSummary(): Promise<MetricsSummary> {
	const res = await fetch(`${API}/api/metrics/summary`);
	if (!res.ok) throw new Error(await res.text());
	return res.json();
}

export async function fetchLatencySeries(): Promise<LatencyPoint[]> {
	const res = await fetch(`${API}/api/metrics/latency`);
	if (!res.ok) throw new Error(await res.text());
	return res.json();
}

export async function fetchLogs(): Promise<LogRow[]> {
	const res = await fetch(`${API}/api/logs`);
	if (!res.ok) throw new Error(await res.text());
	return res.json();
}
