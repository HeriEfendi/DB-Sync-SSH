import { ref } from 'vue';

/**
 * Composable for handling sidebar dragging and collapsing gestures.
 * Complies with AGENTS.md: SoC, SRP (<100 lines).
 */
export function useSidebarDrag() {
  const collapsed = ref(false);
  const COLLAPSE_THRESHOLD = 60; // px dragged to the left to trigger collapse
  let dragStartX = 0;
  let dragActive = false;
  let movedEnough = false;

  function onDragMove(e) {
    if (!dragActive) return;
    const delta = dragStartX - e.clientX;
    if (delta > 8) movedEnough = true;
    if (delta >= COLLAPSE_THRESHOLD && !collapsed.value) {
      collapsed.value = true;
      endDrag();
    }
  }

  function onDragMoveTouch(e) {
    if (!dragActive) return;
    e.preventDefault();
    const delta = dragStartX - e.touches[0].clientX;
    if (delta > 8) movedEnough = true;
    if (delta >= COLLAPSE_THRESHOLD && !collapsed.value) {
      collapsed.value = true;
      endDragTouch();
    }
  }

  function endDrag() {
    dragActive = false;
    window.removeEventListener('mousemove', onDragMove);
    window.removeEventListener('mouseup', endDrag);
  }

  function endDragTouch() {
    dragActive = false;
    window.removeEventListener('touchmove', onDragMoveTouch);
    window.removeEventListener('touchend', endDragTouch);
  }

  function startDrag(e) {
    dragActive = true;
    movedEnough = false;
    dragStartX = e.clientX;
    window.addEventListener('mousemove', onDragMove);
    window.addEventListener('mouseup', endDrag);
  }

  function startDragTouch(e) {
    dragActive = true;
    movedEnough = false;
    dragStartX = e.touches[0].clientX;
    window.addEventListener('touchmove', onDragMoveTouch, { passive: false });
    window.addEventListener('touchend', endDragTouch);
  }

  function onHandleClick() {
    if (!movedEnough) {
      collapsed.value = !collapsed.value;
    }
    movedEnough = false;
  }

  function cleanupDragListeners() {
    window.removeEventListener('mousemove', onDragMove);
    window.removeEventListener('mouseup', endDrag);
    window.removeEventListener('touchmove', onDragMoveTouch);
    window.removeEventListener('touchend', endDragTouch);
  }

  return {
    collapsed,
    startDrag,
    startDragTouch,
    onHandleClick,
    cleanupDragListeners,
  };
}

