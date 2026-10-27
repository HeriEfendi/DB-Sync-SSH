/**
 * appletProgress.js
 * Manages taskbar/dock progress bar, dynamic animated applet/favicon, and window title
 * so users can monitor sync progress from other applications or the taskbar/dock.
 */

let originalTitle = 'DB Sync Engine SSH';
let originalFavicon = '/favicon.svg';
let canvas = null;
let ctx = null;

function getCanvas() {
  if (!canvas && typeof document !== 'undefined') {
    canvas = document.createElement('canvas');
    canvas.width = 64;
    canvas.height = 64;
    ctx = canvas.getContext('2d');
  }
  return { canvas, ctx };
}

/**
 * Draws a circular progress ring (similar to Firefox download animation)
 * and updates the favicon dynamically.
 */
function renderDynamicFavicon(pct) {
  if (typeof document === 'undefined') return;
  const { canvas, ctx } = getCanvas();
  if (!canvas || !ctx) return;

  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  const cx = w / 2;
  const cy = h / 2;
  const radius = 26;
  const strokeWidth = 6;

  // Background circle
  ctx.beginPath();
  ctx.arc(cx, cy, radius, 0, 2 * Math.PI);
  ctx.fillStyle = '#0f172a';
  ctx.fill();

  // Track circle (dim)
  ctx.beginPath();
  ctx.arc(cx, cy, radius - strokeWidth / 2, 0, 2 * Math.PI);
  ctx.lineWidth = strokeWidth;
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.15)';
  ctx.stroke();

  // Animated progress arc (vibrant emerald green)
  const normalizedPct = Math.min(100, Math.max(0, pct));
  const startAngle = -Math.PI / 2;
  const endAngle = startAngle + (2 * Math.PI * (normalizedPct / 100));

  ctx.beginPath();
  ctx.arc(cx, cy, radius - strokeWidth / 2, startAngle, endAngle);
  ctx.lineWidth = strokeWidth;
  ctx.strokeStyle = '#10b981';
  ctx.lineCap = 'round';
  ctx.stroke();

  // Center text (percentage or arrow)
  ctx.fillStyle = '#ffffff';
  ctx.font = 'bold 18px sans-serif';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  if (normalizedPct < 100) {
    ctx.fillText(`${normalizedPct}`, cx, cy);
  } else {
    ctx.fillText('✓', cx, cy);
  }

  const iconUrl = canvas.toDataURL('image/png');
  let link = document.querySelector("link[rel*='icon']");
  if (!link) {
    link = document.createElement('link');
    link.rel = 'icon';
    document.head.appendChild(link);
  }
  link.href = iconUrl;
}

/**
 * Updates taskbar progress, window title, and favicon.
 */
export async function updateAppletProgress(percent, label = '') {
  if (typeof document === 'undefined') return;

  const pct = Math.min(100, Math.max(0, Math.round(percent)));
  const titleText = label
    ? `[${pct}%] ${label} - ${originalTitle}`
    : `[${pct}%] Syncing... - ${originalTitle}`;

  document.title = titleText;
  renderDynamicFavicon(pct);

  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const win = getCurrentWindow();
    if (win?.setProgressBar) {
      await win.setProgressBar({
        progress: pct,
        status: 'normal',
      });
    }
  } catch {
    // Ignore if not running in Tauri or permission unavailable
  }
}

/**
 * Clears taskbar progress, restores original title and default favicon.
 */
export async function clearAppletProgress(finalStatus = '') {
  if (typeof document === 'undefined') return;

  if (finalStatus === 'success') {
    document.title = `[✓ Selesai] ${originalTitle}`;
    setTimeout(() => {
      document.title = originalTitle;
    }, 5000);
  } else if (finalStatus === 'failed') {
    document.title = `[✕ Gagal] ${originalTitle}`;
    setTimeout(() => {
      document.title = originalTitle;
    }, 5000);
  } else {
    document.title = originalTitle;
  }

  // Restore favicon
  const link = document.querySelector("link[rel*='icon']");
  if (link) {
    link.href = originalFavicon;
  }

  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const win = getCurrentWindow();
    if (win?.setProgressBar) {
      await win.setProgressBar({
        progress: null,
        status: 'none',
      });
    }
  } catch {
    // Ignore
  }
}
