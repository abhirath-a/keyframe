export function createViewerState(getPageCount: () => number) {
  let currentPage = 1;

  return {
    get currentPage() {
      return currentPage;
    },

    setPage(page: number) {
      const max = getPageCount();
      if (page >= 1 && page <= max) {
        currentPage = page;
      }
    },

    next() {
      this.setPage(currentPage + 1);
    },

    prev() {
      this.setPage(currentPage - 1);
    },
  };
}
