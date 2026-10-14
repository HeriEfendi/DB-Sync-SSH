import { createApp } from 'vue';
import './styles/main.css';
import App from './App.vue';

// Auto-hide scrollbar: only reveal thumb while scrolling, hide 2x faster when idle
if (typeof window !== 'undefined') {
  window.addEventListener(
    'scroll',
    (e) => {
      const el = e.target;
      if (el && el.classList) {
        el.classList.add('is-scrolling');
        clearTimeout(el._scrollTimer);
        el._scrollTimer = setTimeout(() => {
          el.classList.remove('is-scrolling');
        }, 400); // 2x faster fade out
      }
    },
    { capture: true, passive: true }
  );
}

createApp(App).mount('#app');
