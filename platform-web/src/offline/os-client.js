/* Registration failure must never prevent login or shell startup. */
(() => {
  if (window.top !== window.self || !window.isSecureContext || !('serviceWorker' in navigator)) return;
  navigator.serviceWorker.register('/service-worker.js', { scope: '/', updateViaCache: 'none' }).catch(error => { console.warn('[rumahl OS] Service Worker registration failed:', error); });
})();
