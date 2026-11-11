import { ref, computed, onBeforeUnmount } from 'vue';

/**
 * Composable for managing background auto-synchronization interval and countdown.
 * Complies with AGENTS.md: SoC, SRP (<100 lines).
 */
export function useAutoSync(onTriggerSync) {
  const autoSyncIntervalMinutes = ref(0);
  const autoSyncRemainingSeconds = ref(0);
  let timer = null;

  const autoSyncCountdownText = computed(() => {
    const mins = Math.floor(autoSyncRemainingSeconds.value / 60);
    const secs = autoSyncRemainingSeconds.value % 60;
    return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
  });

  function stopAutoSyncTimer() {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
    autoSyncRemainingSeconds.value = 0;
  }

  function startAutoSyncTimer() {
    stopAutoSyncTimer();
    if (autoSyncIntervalMinutes.value <= 0) return;
    autoSyncRemainingSeconds.value = autoSyncIntervalMinutes.value * 60;

    timer = setInterval(() => {
      autoSyncRemainingSeconds.value -= 1;
      if (autoSyncRemainingSeconds.value <= 0) {
        autoSyncRemainingSeconds.value = autoSyncIntervalMinutes.value * 60;
        if (onTriggerSync) onTriggerSync();
      }
    }, 1000);
  }

  function setAutoSyncInterval(minutes, profileId) {
    autoSyncIntervalMinutes.value = minutes;
    if (profileId) {
      localStorage.setItem(`db_sync_auto_${profileId}`, String(minutes));
    }
    if (minutes > 0) {
      startAutoSyncTimer();
    } else {
      stopAutoSyncTimer();
    }
  }

  onBeforeUnmount(() => {
    stopAutoSyncTimer();
  });

  return {
    autoSyncIntervalMinutes,
    autoSyncCountdownText,
    setAutoSyncInterval,
    stopAutoSyncTimer,
  };
}

