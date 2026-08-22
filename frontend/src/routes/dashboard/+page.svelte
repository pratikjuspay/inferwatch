<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import {
		fetchMetricsSummary,
		fetchLatencySeries,
		fetchLogs,
		type MetricsSummary,
		type LatencyPoint,
		type LogRow
	} from '$lib/api';

	let summary = $state<MetricsSummary | null>(null);
	let series = $state<LatencyPoint[]>([]);
	let logs = $state<LogRow[]>([]);
	let timer: ReturnType<typeof setInterval>;

	async function refresh() {
		[summary, series, logs] = await Promise.all([
			fetchMetricsSummary(),
			fetchLatencySeries(),
			fetchLogs()
		]);
	}

	onMount(async () => {
		await refresh();
		timer = setInterval(refresh, 5000); // live dashboard
	});
	onDestroy(() => clearInterval(timer));

	// SVG chart geometry
	const W = 860;
	const H = 220;
	function path(points: LatencyPoint[]): string {
		if (points.length < 2) return '';
		const xs = points.map((_, i) => (i / (points.length - 1)) * W);
		const ys = points.map((p) => p.avg_latency_ms);
		const max = Math.max(...ys, 1);
		const coords = xs.map((x, i) => `${x},${H - (ys[i] / max) * (H - 30)}`);
		return `M${coords.join(' L')}`;
	}
	function fmtErr(pct: number): string {
		return pct === 0 ? '0%' : `${(pct * 100).toFixed(1)}%`;
	}
</script>

<div class="head">
	<h1>Metrics</h1>
	<span class="live"><span class="dot"></span> live · 5s</span>
</div>

{#if summary}
	<div class="cards">
		<div class="card"><span class="label">Total calls</span><span class="value">{summary.total_calls}</span></div>
		<div class="card">
			<span class="label">Error rate</span>
			<span class="value" class:bad={summary.error_rate > 0.05} class:good={summary.error_rate === 0}>
				{fmtErr(summary.error_rate)}
			</span>
			<span class="sub">{summary.error_count} failed</span>
		</div>
		<div class="card">
			<span class="label">Avg latency</span>
			<span class="value">{summary.avg_latency_ms.toFixed(0)} <small>ms</small></span>
			<span class="sub">p95 {summary.p95_latency_ms?.toFixed(0) ?? '—'} ms</span>
		</div>
		<div class="card"><span class="label">Calls / hour</span><span class="value">{summary.calls_last_hour}</span></div>
		<div class="card">
			<span class="label">Tokens</span>
			<span class="value">{(summary.total_input_tokens + summary.total_output_tokens).toLocaleString()}</span>
			<span class="sub">{summary.total_input_tokens.toLocaleString()} in · {summary.total_output_tokens.toLocaleString()} out</span>
		</div>
	</div>
{/if}

<h2>Latency · last 24h</h2>
{#if series.length >= 2}
	<div class="chart-box">
		<svg viewBox="0 0 {W} {H}" class="chart" preserveAspectRatio="none">
			<defs>
				<linearGradient id="fill" x1="0" y1="0" x2="0" y2="1">
					<stop offset="0%" stop-color="rgba(129,140,248,0.35)" />
					<stop offset="100%" stop-color="rgba(129,140,248,0)" />
				</linearGradient>
			</defs>
			<path d="{path(series)} L{W},{H} L0,{H} Z" fill="url(#fill)" />
			<path d={path(series)} fill="none" stroke="#818cf8" stroke-width="2" stroke-linejoin="round" />
		</svg>
		<div class="axis">
			<span>{new Date(series[0].bucket).toLocaleTimeString()}</span>
			<span>{new Date(series[series.length - 1].bucket).toLocaleTimeString()}</span>
		</div>
	</div>
{:else}
	<p class="muted">Not enough data yet — send a few messages, then watch this chart draw itself.</p>
{/if}

<h2>Recent inference logs</h2>
<div class="table-wrap">
	<table>
		<thead>
			<tr>
				<th>time</th>
				<th>model</th>
				<th class="num">latency</th>
				<th class="num">tokens</th>
				<th>status</th>
				<th>input</th>
				<th>output</th>
			</tr>
		</thead>
		<tbody>
			{#each logs as l (l.id)}
				<tr>
					<td class="muted now">{new Date(l.created_at).toLocaleTimeString()}</td>
					<td class="nowrap">{l.provider}<span class="muted">/{l.model}</span></td>
					<td class="num now">{l.latency_ms}<span class="muted">ms</span></td>
					<td class="num now muted">{l.input_tokens ?? '?'}/{l.output_tokens ?? '?'}</td>
					<td>
						<span class="pill" class:pill-ok={l.status === 'success'} class:pill-err={l.status === 'error'}>
							{l.status}
						</span>
					</td>
					<td class="preview">{l.input_preview ?? ''}</td>
					<td class="preview">{l.output_preview ?? ''}</td>
				</tr>
			{/each}
		</tbody>
	</table>
</div>

<style>
	.head {
		display: flex;
		align-items: baseline;
		gap: 0.9rem;
		margin-bottom: 1rem;
	}
	h1 { font-size: 1.35rem; margin: 0; }
	h2 { font-size: 1.05rem; margin: 1.6rem 0 0.6rem; }
	.live {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		color: var(--muted);
		font-size: 0.78rem;
	}
	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--ok);
		animation: pulse 1.6s ease-in-out infinite;
	}
	@keyframes pulse {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.35; }
	}

	.cards {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(155px, 1fr));
		gap: 0.7rem;
	}
	.card {
		background: var(--panel);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		padding: 0.9rem 1rem;
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		transition: border-color 0.15s;
	}
	.card:hover { border-color: var(--border-strong); }
	.label {
		color: var(--muted);
		font-size: 0.7rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.07em;
	}
	.value { font-size: 1.35rem; font-weight: 700; font-variant-numeric: tabular-nums; }
	.value small { font-size: 0.7em; color: var(--muted); font-weight: 500; }
	.sub { font-size: 0.74rem; color: var(--muted); }
	.bad { color: var(--err); }
	.good { color: var(--ok); }

	.chart-box {
		background: var(--panel);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		padding: 0.8rem 0.8rem 0.4rem;
	}
	.chart { width: 100%; height: 220px; display: block; }
	.axis {
		display: flex;
		justify-content: space-between;
		color: var(--muted);
		font-size: 0.72rem;
		padding: 0.2rem 0.2rem 0.2rem;
	}

	.table-wrap { overflow-x: auto; border: 1px solid var(--border); border-radius: var(--radius); background: var(--panel); }
	table { width: 100%; border-collapse: collapse; font-size: 0.82rem; }
	th {
		text-align: left;
		color: var(--muted);
		font-weight: 600;
		font-size: 0.7rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		padding: 0.7rem 0.9rem;
		border-bottom: 1px solid var(--border);
		white-space: nowrap;
		position: sticky;
		top: 0;
		background: var(--panel);
	}
	td {
		padding: 0.65rem 0.9rem;
		border-bottom: 1px solid var(--border);
		vertical-align: top;
	}
	tbody tr:last-child td { border-bottom: none; }
	tbody tr:hover { background: var(--panel-2); }
	.num { text-align: right; font-variant-numeric: tabular-nums; }
	.now { white-space: nowrap; }
	.nowrap { white-space: nowrap; }
	.pill {
		font-size: 0.72rem;
		font-weight: 600;
		padding: 0.15rem 0.6rem;
		border-radius: 999px;
	}
	.pill-ok { background: rgba(98, 210, 162, 0.12); color: var(--ok); }
	.pill-err { background: rgba(243, 135, 135, 0.12); color: var(--err); }
	.muted { color: var(--muted); }
	.preview {
		max-width: 240px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--muted);
	}

	@media (max-width: 640px) {
		.cards { grid-template-columns: repeat(2, 1fr); }
		.preview { display: none; }
	}
</style>
