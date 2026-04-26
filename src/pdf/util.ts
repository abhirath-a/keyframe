export const clamp = (value: number, min: number, max: number) =>
	Math.max(min, Math.min(value, max));

// biome-ignore lint/suspicious/noExplicitAny: meant to accept *any* function
export function debounce<F extends (...args: any[]) => void>(
	fn: F,
	delay: number,
) {
	let t: number;

	return (...args: Parameters<F>) => {
		clearTimeout(t);
		t = window.setTimeout(() => fn(...args), delay);
	};
}
