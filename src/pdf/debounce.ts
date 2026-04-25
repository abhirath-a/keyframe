export function debounce<F extends (...args: any[]) => void>(
  fn: F,
  delay: number
) {
  let t: number;

  return (...args: Parameters<F>) => {
    clearTimeout(t);
    t = window.setTimeout(() => fn(...args), delay);
  };
}
