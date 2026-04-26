type PageCache = Map<string, ImageBitmap>;

export function createPageCache() {
	const cache: PageCache = new Map();

	function get(page: string): ImageBitmap | undefined {
		return cache.get(page);
	}

	function set(page: string, bitmap: ImageBitmap) {
		const prev = cache.get(page);
		if (prev) prev.close();

		cache.set(page, bitmap);
	}

	function has(page: string): boolean {
		return cache.has(page);
	}

	function clear() {
		for (const bmp of cache.values()) {
			bmp.close();
		}

		cache.clear();
	}

	return {
		get,
		set,
		has,
		clear,
	};
}
