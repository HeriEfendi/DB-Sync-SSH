import { ref, computed } from 'vue';

/**
 * Composable for high-performance virtual scrolling.
 * Complies with AGENTS.md: SoC, SRP (<100 lines).
 */
export function useVirtualScroll(itemsRef, rowHeight = 32, bufferItems = 15) {
  const containerRef = ref(null);
  const scrollTop = ref(0);
  const containerHeight = ref(500);

  const isVirtualScrollActive = computed(() => itemsRef.value.length > 50);

  const startIndex = computed(() => {
    if (!isVirtualScrollActive.value) return 0;
    const index = Math.floor(scrollTop.value / rowHeight) - bufferItems;
    return Math.max(0, index);
  });

  const endIndex = computed(() => {
    const total = itemsRef.value.length;
    if (!isVirtualScrollActive.value) return total;
    const count = Math.ceil(containerHeight.value / rowHeight) + 2 * bufferItems;
    return Math.min(total, startIndex.value + count);
  });

  const visibleItems = computed(() => {
    if (!isVirtualScrollActive.value) return itemsRef.value;
    return itemsRef.value.slice(startIndex.value, endIndex.value);
  });

  const topSpacerHeight = computed(() => {
    if (!isVirtualScrollActive.value) return 0;
    return startIndex.value * rowHeight;
  });

  const bottomSpacerHeight = computed(() => {
    if (!isVirtualScrollActive.value) return 0;
    const total = itemsRef.value.length;
    return Math.max(0, (total - endIndex.value) * rowHeight);
  });

  let rafId = null;
  function handleScroll(e) {
    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
      if (e.target) {
        scrollTop.value = e.target.scrollTop;
      }
    });
  }

  function resetScroll() {
    if (containerRef.value) {
      containerRef.value.scrollTop = 0;
    }
    scrollTop.value = 0;
  }

  return {
    containerRef,
    visibleItems,
    topSpacerHeight,
    bottomSpacerHeight,
    handleScroll,
    resetScroll,
  };
}

