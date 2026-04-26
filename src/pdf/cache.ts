type PageCache = Map<string, string>;

export function createPageCache() {
	const cache: PageCache = new Map();

	function get(page: string): string | undefined {
		return cache.get(page);
	}

	function set(page: string, data: string) {
		cache.set(page, data);
	}

	function has(page: string): boolean {
		return cache.has(page);
	}

	function clear() {
		cache.clear();
	}

	return {
		get,
		set,
		has,
		clear,
	};
}
