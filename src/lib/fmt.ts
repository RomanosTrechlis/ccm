export function num(n: number | null | undefined): string {
	const v = n ?? 0;
	if (v >= 1e9) return (v / 1e9).toFixed(2) + 'B';
	if (v >= 1e6) return (v / 1e6).toFixed(2) + 'M';
	if (v >= 1e3) return (v / 1e3).toFixed(1) + 'k';
	return String(Math.round(v));
}

export function usd(n: number | null | undefined): string {
	return '$' + (n ?? 0).toFixed(2);
}

export function mins(m: number): string {
	if (m >= 60) return `${Math.floor(m / 60)}h ${String(Math.round(m % 60)).padStart(2, '0')}m`;
	return `${Math.round(m)}m`;
}

export function timeAgo(iso: string | undefined): string {
	if (!iso) return '—';
	const m = (Date.now() - new Date(iso).getTime()) / 60000;
	if (m < 1) return 'just now';
	if (m < 60) return `${Math.round(m)}m ago`;
	if (m < 60 * 24) return `${Math.round(m / 60)}h ago`;
	return `${Math.round(m / 60 / 24)}d ago`;
}

/** "claude-haiku-4-5-20251001" -> "haiku-4-5" */
export function modelShort(m: string): string {
	return m.replace(/^claude-/, '').replace(/-\d{8}$/, '');
}

/** local date as YYYY-MM-DD, matching ccusage daily rows */
export function todayStr(): string {
	const d = new Date();
	return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

/** "2026-07-06" -> "Mon, Jul 6" (parsed as local date, not UTC) */
export function dayLabel(dateStr: string): string {
	const [y, m, d] = dateStr.split('-').map(Number);
	return new Date(y, m - 1, d).toLocaleDateString(undefined, {
		weekday: 'short',
		month: 'short',
		day: 'numeric'
	});
}

/** shared green/amber/red/grey band — mirrors gauge.rs color_band */
export function bandColor(
	gauge: { active: boolean; percent: number } | null,
	hasError: boolean
): string {
	if (hasError || !gauge?.active) return 'var(--muted)';
	if (gauge.percent >= 0.85) return 'var(--red)';
	if (gauge.percent >= 0.6) return 'var(--amber)';
	return 'var(--green)';
}

export function daysAgoStr(days: number): string {
	const d = new Date(Date.now() - days * 86400000);
	return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}
