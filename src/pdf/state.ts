export function createViewerState(getPageCount: () => number) {
  let currentPage = 1;
  let fullscreen = true;

  function setPage(page: number) {
    const max = getPageCount();
    if (page >= 1 && page <= max) {
      currentPage = page;
    }
  }

  return {
    get currentPage() {
      return currentPage;
    },
    get fullscreen() {
      return fullscreen;
    },
    toggleFullscreen() {
      fullscreen = !fullscreen;    
    },

    setPage,

    next() {
      setPage(currentPage + 1);
    },

    prev() {
      setPage(currentPage - 1);
    },
  };
}
