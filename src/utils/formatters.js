/**
 * Pure helper utility functions for formatting numbers, dates, IDs, and statuses.
 * Complies with AGENTS.md: Pure helper extraction & DRY principle.
 */

export function formatNumber(n) {
  if (n === undefined || n === null) return '0';
  return n.toString().replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}

export function formatLastId(id) {
  if (id === null || id === undefined || id === '') return '-';
  return String(id).replace(/\B(?=(\d{3})+(?!\d))/g, '.');
}

export function getSyncDate(timeStr, epochMs) {
  if (epochMs !== undefined && epochMs !== null && Number.isFinite(Number(epochMs))) {
    const date = new Date(Number(epochMs));
    if (!isNaN(date.getTime())) return date;
  }
  if (!timeStr) return null;
  try {
    const date = new Date(timeStr.includes(' ') ? timeStr.replace(' ', 'T').slice(0, 23) : timeStr);
    return isNaN(date.getTime()) ? null : date;
  } catch {
    return null;
  }
}

export function formatDisplayTime(timeStr, epochMs) {
  const date = getSyncDate(timeStr, epochMs);
  if (!date) return '';
  const parts = new Intl.DateTimeFormat('id-ID', {
    timeZone: 'Asia/Jakarta',
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hourCycle: 'h23',
  }).formatToParts(date);
  const part = (type) => parts.find((item) => item.type === type)?.value || '';
  return `${part('day')}/${part('month')} ${part('hour')}:${part('minute')}`;
}

export function formatFullDate(timeStr, epochMs) {
  const date = getSyncDate(timeStr, epochMs);
  if (!date) return '';
  return new Intl.DateTimeFormat('id-ID', {
    timeZone: 'Asia/Jakarta',
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hourCycle: 'h23',
    timeZoneName: 'short',
  }).format(date);
}

export function formatDurationMs(durationMs) {
  const durationSec = Math.round((durationMs || 0) / 1000);
  const mins = Math.floor(durationSec / 60);
  const secs = durationSec % 60;
  return mins > 0 ? `${mins}m ${secs}s` : `${secs}s`;
}

export function extractTableName(msg) {
  if (!msg) return null;
  const match = msg.match(/\[Tabel ['"]?([^'"\]]+)['"]?\]/);
  return match ? match[1] : null;
}

export function historyStatus(status) {
  if (status === 'success') return 'Completed';
  if (status === 'partial') return 'Partial';
  if (status === 'cancelled') return 'Cancelled';
  return 'Failed';
}

