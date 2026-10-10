(() => {
  const de = navigator.language.startsWith('de');
  if (de) {
    document.documentElement.lang = 'de';
    document.getElementById('title').textContent = 'Du bist offline';
    document.getElementById('description').textContent = 'Die Verbindung zu rumahl OS ist unterbrochen. Anmeldung und persönliche Apps benötigen eine Verbindung zu diesem Server.';
    document.getElementById('retry').textContent = 'Erneut versuchen';
    document.getElementById('status').textContent = 'Wir verbinden dich wieder, sobald der Server erreichbar ist.';
  }
  const destination = location.pathname === '/offline' ? '/' : location.pathname + location.search;
  const retry = document.getElementById('retry');
  retry.href = destination;
  let checking = false;
  async function reconnect() {
    if (checking || document.hidden) return;
    checking = true;
    try {
      const response = await fetch('/offline/ping', { credentials: 'omit', cache: 'no-store', signal: AbortSignal.timeout(5000) });
      if (response.ok) location.replace(destination);
    } catch { /* Remain offline; do not replay any user action. */ }
    finally { checking = false; }
  }
  window.addEventListener('online', reconnect);
  document.addEventListener('visibilitychange', () => { if (!document.hidden) void reconnect(); });
  setInterval(reconnect, 10000);
})();
