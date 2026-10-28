/* Standalone recovery: deliberately independent of the shell and its bundles. */
(function () {
  'use strict';
  // Install the offline kit so later failures can distinguish "this device is
  // offline" from "rumahl OS is unreachable" via the worker's network probe.
  if ('serviceWorker' in navigator && (location.protocol === 'https:' || location.hostname === 'localhost' || location.hostname === '127.0.0.1')) {
    navigator.serviceWorker.register('/service-worker.js', { scope: '/' }).catch(() => { /* optional */ });
  }
  // Extensible recovery locales. English is the source language (the lookup
  // key); other languages are added per key in `rre-i18n.json`. Add a language
  // by adding one entry — no code change required.
  const I18N = (window.RRE_I18N && window.RRE_I18N.locales) || {};
  const LANGUAGE = (navigator.language || 'en').slice(0, 2).toLowerCase();
  const t = key => (I18N[LANGUAGE] && I18N[LANGUAGE][key]) || key;
  const el = (tag, text, parent) => {
    const node = document.createElement(tag);
    if (text != null) node.textContent = text;
    if (parent) parent.append(node);
    return node;
  };
  document.querySelectorAll('[data-i18n]').forEach(node => { node.textContent = t(node.dataset.i18n); });
  const main = document.querySelector('main');
  const notice = document.getElementById('operation-status');
  notice.setAttribute('role', 'status');
  notice.setAttribute('aria-live', 'polite');

  const report = {};
  let taskQueue = Promise.resolve();
  async function api(path, body, method) {
    const response = await fetch(path, {
      credentials: 'same-origin', cache: 'no-store', signal: AbortSignal.timeout(20000),
      method: method || (body === undefined ? 'GET' : 'POST'),
      headers: body === undefined ? {} : { 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body)
    });
    if (!response.ok) {
      const failure = new Error(response.status === 401 ? t('Please sign in.') : `${t('Request failed')} (${response.status})`);
      failure.status = response.status;
      throw failure;
    }
    return response.status === 204 ? null : response.json();
  }
  function confirmAction(title, description) {
    return new Promise(resolve => {
      const dialog = el('dialog', null, document.body);
      const heading = el('h2', title, dialog); heading.id = 'confirmation-title';
      dialog.setAttribute('aria-labelledby', heading.id);
      el('p', description || t('The app connection will be interrupted. Unsaved work may be lost.'), dialog);
      const cancel = el('button', t('Cancel'), dialog); cancel.className = 'secondary'; cancel.type = 'button';
      const accept = el('button', t('Continue'), dialog); accept.className = 'secondary'; accept.type = 'button';
      let accepted = false;
      cancel.onclick = () => dialog.close();
      accept.onclick = () => { accepted = true; dialog.close(); };
      dialog.addEventListener('close', () => { dialog.remove(); resolve(accepted); }, { once: true });
      dialog.showModal(); cancel.focus();
    });
  }
  function run(task) {
    const next = taskQueue.then(async () => {
      notice.textContent = t('Working…');
      try { await task(); if (notice.textContent === t('Working…')) notice.textContent = ''; }
      catch (error) { notice.textContent = error.message; }
    });
    taskQueue = next;
    return next;
  }
  function button(parent, title, action) {
    const node = el('button', title, parent);
    node.type = 'button'; node.className = 'secondary';
    node.addEventListener('click', () => { node.disabled = true; run(action).finally(() => { node.disabled = Boolean(node.closest('[data-maintenance-pending]')); }); });
    return node;
  }
  function page(name) {
    const entry = windows.get(name);
    if (!entry) return;
    entry.frame.hidden = false; entry.opened = true;
    entry.dock.classList.add('is-open');
    fitWindow(entry); focusWindow(entry);
  }
  function host(id) {
    const node = document.getElementById(id);
    node.replaceChildren(); node.hidden = false;
    return node;
  }
  function group(parent, title, description) {
    const section = el('section', null, parent); section.className = 'app-group';
    el('h3', title, section);
    if (description) el('p', description, section).className = 'group-description';
    return section;
  }
  function statusBadge(parent, value) {
    const labels = { running: t('Running'), stopped: t('Stopped'), unknown: t('Not checked'), disabled: t('Unavailable'), warning: t('Warning'), error: t('Error'), information: t('Normal') };
    const badge = el('span', labels[value] || value, parent); badge.className = 'state-badge'; badge.dataset.state = value;
    return badge;
  }
  function facts(parent, pairs) {
    const list = el('dl', null, parent); list.className = 'facts';
    pairs.forEach(([label, value]) => { const item = el('div', null, list); el('dt', label, item); el('dd', value == null ? '—' : String(value), item); });
  }
  function actions(parent) { const row = el('div', null, parent); row.className = 'app-actions'; return row; }
  function bytes(value) { return typeof value === 'number' && Number.isFinite(value) ? `${(value / 1073741824).toFixed(1)} GB` : '—'; }
  async function diagnose() {
    const node = host('problems-host');
    el('p', t('Checking…'), node);
    try {
      const results = await Promise.allSettled(['diagnostics', 'status', 'sensors', 'storage'].map(async key => {
        const data = await api(`/api/v1/recovery/${key}`); report[key] = data; return { key, data };
      }));
      node.replaceChildren();
      report.checkedAt = new Date().toISOString();
      const incomplete = results.some(result => result.status === 'rejected');
      const problems = results[0].status === 'fulfilled' ? results[0].value.data.problems : [];
      document.querySelector('.summary').dataset.health = incomplete || problems.length ? 'attention' : 'checked';
      document.getElementById('health-title').textContent = incomplete ? t('Checks incomplete') : problems.length ? t('Your system needs attention') : t('Basic checks completed');
      document.getElementById('health-summary').textContent = incomplete ? t('At least one check was unavailable. The results are incomplete.') : `${problems.length} ${t('findings · Last checked: ')}${new Date().toLocaleTimeString()}`;
      const overview = host('overview-facts');
      facts(overview, [[t('Checks available'), `${results.filter(r => r.status === 'fulfilled').length} / ${results.length}`], [t('Findings'), results[0].status === 'fulfilled' ? problems.length : '—'], [t('Last checked'), new Date().toLocaleTimeString()]]);
      const findings = group(node, t('Findings'));
      const checks = host('overview-checks');
      const labels = { diagnostics: t('System check'), status: t('Platform'), sensors: t('Resources'), storage: t('Storage') };
      results.forEach((result, index) => { const row = el('div', null, checks); row.className = 'service-row'; el('span', labels[['diagnostics', 'status', 'sensors', 'storage'][index]], row); statusBadge(row, result.status === 'fulfilled' ? 'information' : 'error'); });
      if (incomplete) { const login = el('a', t('Sign in'), node); login.href = '/login'; }

      results.forEach((result, index) => {
        if (result.status === 'rejected') {
          const key = ['diagnostics', 'status', 'sensors', 'storage'][index];
          report[key] = { error: result.reason.message };
          el('p', `${key}: ${result.reason.message}`, findings).className = 'inline-error';
        } else if (result.value.key === 'diagnostics') {
          const problems = result.value.data.problems;
          if (!problems.length) el('p', t('No problems detected by these checks.'), findings).className = 'empty-state';
          problems.forEach(problem => {
            const row = el('div', null, findings); row.className = 'problem';
            statusBadge(row, problem.severity);
            const labels = {
              'app-check': [t('App services could not be checked'), t('The app catalog is unavailable or access was denied.')],
              audit: [t('The activity log is unavailable'), t('The audit store is not connected. Check the platform service configuration.')],
              apps: [t('The app runtime is unavailable'), t('The supervisor is not connected. Reloading the desktop will not fix this.')],
              sensors: [t('High resource pressure'), t('Check the resource readings and the current load.')],
              storage: [t('Low free storage'), t('Check disk usage. No files are deleted automatically.')]
            };
            const copy = labels[problem.id];
            el('strong', copy ? copy[0] : problem.title, row); el('p', copy ? copy[1] : problem.detail, row);
            if (problem.fix === 'self-heal') button(row, t('Repair app services'), repair);

          });
        }
      });

      for (const key of ['sensors', 'storage']) {
        const details = el('details', null, node); details.className = 'panel';
        details.classList.add('app-group');
        el('summary', key === 'sensors' ? t('Resource readings') : t('Storage usage'), details);
        const data = report[key];
        if (key === 'sensors' && data.metrics) data.metrics.forEach(metric => { const row = el('div', null, details); row.className = 'metric-row'; el('span', metric.label, row); el('strong', metric.value == null ? '—' : `${metric.value}${metric.unit || ''}`, row); statusBadge(row, metric.level); });
        else if (key === 'storage' && !data.error) facts(details, [[t('Mount'), data.mount], [t('Used'), data.percent == null ? '—' : `${data.percent}%`], [t('Available'), bytes(data.available)]]);
        else el('p', data.error || t('Unavailable'), details);
      }
      const commands = actions(node);
      button(commands, t('Check again'), diagnose);
      button(commands, t('Repair app services'), repair);
      button(commands, t('Download diagnostic report'), download);
    } catch (error) { node.replaceChildren(); el('p', error.message, node); throw error; }
  }
  function download() {
    const url = URL.createObjectURL(new Blob([JSON.stringify(report, null, 2)], { type: 'application/json' }));
    const link = el('a'); link.href = url; link.download = 'rumahl-rre-diagnostics.json'; link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    notice.textContent = t('Diagnostic report exported.');
  }
  async function repair() {
    const result = await api('/api/v1/recovery/self-heal', {});
    report.repair = result;
    await diagnose();
    const node = group(document.getElementById('problems-host'), t('Repair results'));
    if (!result.actions.length) el('p', t('No repairable app services found.'), node).className = 'empty-state';
    result.actions.forEach(action => {
      const outcomes = { success: t('Repaired'), skipped: t('Skipped'), failure: t('Failed') };
      const messages = { 'already running': t('Already running'), 'safe mode enabled': t('Safe mode enabled'), 'service started': t('Service started'), 'service could not be started': t('Service could not be started'), 'runtime status unavailable or denied': t('Status unavailable or access denied') };
      const line = el('p', `${action.appId}: ${outcomes[action.outcome]} — ${messages[action.detail] || action.detail}`, node);
      line.className = 'outcome' + (action.outcome === 'failure' ? ' error' : '');
    });
    notice.textContent = !result.ok ? t('Some repairs failed.') : result.actions.some(a => a.outcome === 'success') ? t('Stopped app services started.') : t('No automatic change needed or possible.');
  }
  async function components() {
    const node = host('recovery-detail');
    const data = await api('/api/v1/recovery/components');
    const system = group(node, t('System services'));
    data.components.forEach(c => {
      const labels = { shell: t('Desktop'), platform: t('Platform service'), renderer: t('Renderer'), audit: t('Activity log'), apps: t('App supervisor') };
      const row = el('div', null, system); row.className = 'service-row';
      el('span', labels[c.id] || c.id, row);
      statusBadge(row, c.state);

    });
    el('p', t('Platform and renderer cannot be restarted through this interface. Use the host service manager.'), node);
    const applications = group(node, t('Installed applications'));
    const search = el('input', null, applications); search.type = 'search'; search.className = 'app-search'; search.placeholder = t('Filter apps…'); search.setAttribute('aria-label', t('Filter apps'));
    const appRows = [];
    const noMatches = el('p', t('No matching apps.'), applications); noMatches.className = 'empty-state'; noMatches.hidden = true;
    search.addEventListener('input', () => { const query = search.value.trim().toLocaleLowerCase(); appRows.forEach(item => { item.row.hidden = !item.text.includes(query); }); noMatches.hidden = appRows.some(item => !item.row.hidden); });
    const catalog = await api('/api/v1/shell/apps');
    if (!catalog.apps.length) { el('p', t('No accessible apps.'), applications).className = 'empty-state'; search.hidden = true; }
    for (const app of catalog.apps) {
      const row = el('div', null, applications); row.className = 'application-row';
      appRows.push({ row, text: `${app.title} ${app.id}`.toLocaleLowerCase() });
      el('strong', app.title, row);
      el('p', `${app.id} · ${app.lifecycle === 'always-on' ? t('Background service') : t('Application')}`, row);
      const path = `/api/v1/shell/apps/${encodeURIComponent(app.id)}/runtime`;
      const state = statusBadge(row, 'unknown');
      const commands = actions(row);
      function updateState(value) { const replacement = statusBadge(row, value); state.textContent = replacement.textContent; state.dataset.state = value; replacement.remove(); }
      try { updateState((await api(`${path}?installationId=${encodeURIComponent(app.installationId)}`)).state); }
      catch (error) { state.textContent = error.message; state.dataset.state = 'error'; }
      for (const action of ['start', 'stop', 'restart']) {
        button(commands, ({ start: t('Start'), stop: t('Stop'), restart: t('Restart') })[action], async () => {
          if (action !== 'start' && !await confirmAction(`${app.title}: ${action === 'stop' ? t('Stop') : t('Restart')}?`)) { notice.textContent = t('Cancelled.'); return; }
          try {
          const result = action === 'restart' ? await api('/api/v1/recovery/restart', { target: 'app', appId: app.id, installationId: app.installationId }) : await api(path, { action, installationId: app.installationId });
          updateState(result.state);
          notice.textContent = `${app.title}: ${result.state}`;
          } catch (error) { state.textContent = error.message; state.dataset.state = 'error'; throw error; }
        });
      }
    }
  }
  async function settings() {
    const node = host('settings-host');
    await bootActions(node);
    let value = await api('/api/v1/recovery/settings');
    el('p', value.safeMode ? t('Safe mode is active: new app starts are blocked.') : t('Normal operation mode.'), node);
    for (const key of ['autoRepair']) {
      const section = group(node, key === 'safeMode' ? t('Protected startup') : t('Automatic recovery'));
      const label = key === 'safeMode' ? t('Safe mode') : t('Automatically repair app crashes');
      const row = el('div', null, section); row.className = 'setting-row';
      const copy = el('div', null, row);
      const title = el('strong', label, copy); title.id = `setting-${key}`;
      const description = el('p', key === 'safeMode'
        ? t('Prevents new app starts. Existing apps keep running.')
        : t('Attempts to restart crashed apps using your permissions.'), copy);
      description.id = `setting-description-${key}`;
      const result = el('p', '', copy); result.className = 'local-result'; result.setAttribute('role', 'status');
      const control = button(row, '', async () => {
        const next = { ...value, [key]: !value[key] };
        try {
          await api('/api/v1/recovery/settings', next, 'PUT');
          value = next; update(); result.textContent = t('Saved.');
        } catch (error) { result.textContent = error.message; throw error; }
      });
      control.className = 'setting-switch';
      control.setAttribute('role', 'switch');
      control.setAttribute('aria-labelledby', title.id);
      control.setAttribute('aria-describedby', description.id);
      function update() { control.setAttribute('aria-checked', String(value[key])); }

      update();
    }
  }
  async function disks() {
    const node = host('disks-host');
    const data = await api('/api/v1/recovery/disks');
    if (!data.devices.length) el('p', t('No disks reported by the host.'), node).className = 'empty-state';
    data.devices.forEach(disk => {
      const row = group(node, disk.model || `/dev/${disk.name}`, `/dev/${disk.name}`);
      row.classList.add('disk-card');
      facts(row, [[t('Capacity'), bytes(disk.size)], [t('Type'), disk.kind], [t('Mount'), disk.mount], [t('Filesystem'), disk.fstype]]);
      if (typeof disk.percent === 'number' && Number.isFinite(disk.percent)) {
        const usage = el('div', null, row); usage.className = 'disk-usage';
        el('span', `${disk.percent}% ${t('used')}`, usage);
        const meter = el('meter', null, usage); meter.min = 0; meter.max = 100; meter.low = 75; meter.high = 90; meter.optimum = 0; meter.value = disk.percent;
        meter.setAttribute('aria-label', `${disk.name}: ${t('Storage usage')}`);
      } else el('p', t('Usage unavailable.'), row).className = 'group-description';
      if (disk.kind === 'disk') {
        const result = el('p', '', row); result.className = 'local-result'; result.setAttribute('role', 'status');
        button(actions(row), t('Start SMART short test'), async () => {
          result.textContent = t('Requesting test…');
          try {
            await api('/api/v1/recovery/disk-test', { name: disk.name, test: 'short' });
            result.textContent = t('Test started. A completed result is not yet available.');
          } catch (error) { result.textContent = error.message; throw error; }
        });
      }
    });
    const fsck = group(node, t('Filesystem check'), t('Read-only check of the disks. Mounted filesystems are reported as "mounted" and require a maintenance reboot.'));
    const fsckResult = el('div', null, fsck); fsckResult.className = 'local-result'; fsckResult.setAttribute('role', 'status');
    button(actions(fsck), t('Run check'), async () => {
      fsckResult.textContent = t('Checking…');
      const result = await api('/api/v1/recovery/host', { action: 'fsck' });
      fsckResult.replaceChildren();
      const labels = { mounted: t('Mounted – needs maintenance'), unavailable: t('Unavailable') };
      (result.report || []).forEach(entry => {
        const row = el('div', null, fsckResult); row.className = 'check-row';
        const left = el('div', null, row);
        el('strong', entry.device, left);
        if (entry.detail) el('small', String(entry.detail).split('\n')[0], left).className = 'group-description';
        statusBadge(row, entry.state !== 'checked' ? 'warning' : entry.ok ? 'information' : 'error');
        if (labels[entry.state]) left.querySelector('strong').append(` · ${labels[entry.state]}`);
      });
      if (!(result.report || []).length) fsckResult.textContent = t('No disks found.');
    });
  }
  async function hostState(parent) {
    try {
      const status = await api('/api/v1/recovery/host');
      if (status.restorePending) {
        el('p', t('A restore was interrupted. The device remains locked for manual maintenance.'), parent).className = 'inline-error';
      }
      if (status.job) {
        const labels = { running: t('Running'), complete: t('Complete'), failed: t('Failed') };
        el('p', `${t('Last operation')}: ${status.job.action} · ${labels[status.job.state] || status.job.state}`, parent).className = 'local-result';
      }
      return status;
    } catch (error) {
      const message = error && error.status === 503
        ? t('The host maintenance service is not available. On the development server, host maintenance requires a rumahl OS image.')
        : t('Host maintenance is unavailable or not authorized. A maintenance operator must be configured on this device.');
      el('p', message, parent).className = 'inline-error';
      return null;
    }
  }
  async function maintenance(parent, request, title, description) {
    if (!await confirmAction(title, description)) return;
    // Mirror what the server will signal (server authoritatively also sends a
    // lifecycle event): a restart or a power-off, never guessed from timeouts.
    if (['reboot', 'restore', 'reset', 'reinstall', 'rollback', 'poweroff'].includes(request.action)) {
      const state = request.action === 'poweroff' ? 'shutting-down' : 'restarting';
      try { localStorage.setItem('rumahl.lifecycle', JSON.stringify({ state, at: Date.now() })); } catch (error) { /* ignore */ }
    }
    const result = await api('/api/v1/recovery/host', request);
    if (!result.accepted) throw new Error(t('Operation was not accepted.'));
    const message = t('Operation accepted. The connection will be interrupted during maintenance. Reopen RRE when the device returns and check the operation status.');
    el('p', message, parent).className = 'local-result';
    notice.textContent = message;
    parent.dataset.maintenancePending = 'true';
    parent.querySelectorAll('button').forEach(control => { control.disabled = true; });
  }
  async function bootActions(parent) {
    const section = group(parent, t('Restart device'), t('Ends all running sessions and reboots the entire device.'));
    const status = await hostState(section);
    if (!status || status.busy || status.restorePending) return;
    const controls = actions(section);
    button(controls, t('Restart normally'), () => maintenance(section, { action: 'reboot', safeMode: false }, t('Restart device?'), t('All apps will stop. Unsaved work will be lost. rumahl will boot in normal mode.')));
    button(controls, t('Restart in safe mode'), () => maintenance(section, { action: 'reboot', safeMode: true }, t('Restart in safe mode?'), t('All apps will stop. The next boot leaves the app supervisor and container engine disabled. Safe mode remains active until a normal restart.')));
  }
  async function backups() {
    const node = host('backups-host');
    const status = await hostState(node);
    if (!status) return;
    const create = group(node, t('Complete state backup'), t('Accounts, personal data, apps and container state. The device is temporarily unavailable while backing up.'));
    if (!status.backupAvailable) el('p', t('Required data volumes are not all available. Complete backups are currently disabled on this device.'), create).className = 'inline-error';
    if (status.backupAvailable && !status.busy && !status.restorePending) button(actions(create), t('Create backup'), () => maintenance(create, { action: 'backup' }, t('Create backup?'), t('All rumahl services and containers will stop for a consistent snapshot and restart afterwards. Save unsaved work first.')));
    const list = group(node, t('Saved backups'), t('Stored locally on this device. Restore requires the same device, configuration and system build.'));
    if (!status.backups.length) el('p', t('No complete backups yet.'), list).className = 'empty-state';
    status.backups.forEach(backup => {
      const row = el('div', null, list); row.className = 'application-row';
      el('strong', new Date(backup.at * 1000).toLocaleString(), row);
      el('p', backup.id, row);
      el('p', backup.compatible ? t('Matches this device and system build') : t('Incompatible with this device or system build'), row);
      if (status.backupAvailable && backup.compatible && !status.busy && !status.restorePending) button(actions(row), t('Restore…'), () => maintenance(row, { action: 'restore', backupId: backup.id }, t('Restore this state?'), t('The current state will be replaced after creating an additional backup. Newer data and app changes will only be available in that backup. The device reboots in safe mode and all previous login sessions are invalidated.')));
    });
    const schedule = group(node, t('Automatic backups'), t('Creates a complete backup at a fixed interval. The device is briefly unavailable while backing up.'));
    const current = status.schedule || { enabled: false, intervalHours: 24, keep: 3 };
    const summary = el('p', scheduleSummary(current), schedule);
    const form = el('div', null, schedule); form.className = 'app-actions';
    const interval = el('select', null, form); interval.className = 'secondary'; interval.setAttribute('aria-label', t('Interval'));
    [[6, '6 h'], [12, '12 h'], [24, '24 h'], [168, t('Week')]].forEach(([value, label]) => { const option = el('option', label, interval); option.value = String(value); });
    interval.value = String(current.intervalHours);
    const keep = el('select', null, form); keep.className = 'secondary'; keep.setAttribute('aria-label', t('Retention'));
    [1, 2, 3, 5, 10].forEach(value => { const option = el('option', `${t('keep')} ${value}`, keep); option.value = String(value); });
    keep.value = String(current.keep);
    button(form, current.enabled ? t('Disable') : t('Enable'), async () => {
      const result = await api('/api/v1/recovery/host', { action: 'schedule', enabled: !current.enabled, intervalHours: Number(interval.value), keep: Number(keep.value) });
      notice.textContent = t('Schedule saved.');
      summary.textContent = scheduleSummary(result.schedule);
    });
  }
  function scheduleSummary(value) {
    return value.enabled ? `${t('Active')} · ${t('every')} ${value.intervalHours} h · ${t('keep')} ${value.keep}` : t('Inactive');
  }
  async function activity() {
    const node = host('activity-host');
    const filter = el('label', t('Outcome '), node);
    const select = el('select', null, filter); select.className = 'secondary';
    for (const [value, label] of [['', t('All')], ['success', t('Success')], ['failure', t('Failure')], ['denied', t('Denied')]]) {
      const option = el('option', label, select); option.value = value;
    }
    const listing = el('div', null, node); listing.className = 'scroll';
    const paging = actions(node);
    let offset = 0;
    async function load(next) {
      const query = new URLSearchParams({ limit: '50', offset: String(next) });
      if (select.value) query.set('outcome', select.value);
      select.disabled = true;
      try {
        const data = await api(`/api/v1/recovery/audit?${query}`);
        offset = next; listing.replaceChildren(); paging.replaceChildren();
        if (!data.events.length) el('p', t('No matching events.'), listing).className = 'empty-state';
        else {
          const table = el('table', null, listing);
          const header = el('tr', null, el('thead', null, table));
          [t('Time'), t('Action'), t('Outcome'), t('Source / target')].forEach(title => el('th', title, header));
          const body = el('tbody', null, table);
          data.events.forEach(event => {
            const row = el('tr', null, body);
            el('td', new Date(event.at * 1000).toLocaleString(), row);
            el('td', event.action, row);
            el('td', ({ success: t('Success'), failure: t('Failure'), denied: t('Denied') })[event.outcome] || event.outcome, row);
            const source = el('td', event.actor, row); el('small', event.target || '—', source).className = 'event-target';
          });
        }
        if (offset > 0) button(paging, t('Newer'), () => load(Math.max(0, offset - 50)));
        if (data.nextOffset != null) button(paging, t('Older'), () => load(data.nextOffset));
      } catch (error) {
        listing.replaceChildren(); paging.replaceChildren();
        el('p', error.message, listing).className = 'inline-error'; throw error;
      } finally { select.disabled = false; }
    }
    select.addEventListener('change', () => run(() => load(0)));
    await load(0);
  }
  async function network() {
    const node = host('network-host');
    const data = await api('/api/v1/recovery/network');
    const overview = group(node, t('Connectivity'));
    facts(overview, [[t('Hostname'), data.hostname || '—'], [t('Gateway'), data.gateway || '—'], [t('DNS servers'), data.dns && data.dns.length ? data.dns.join(', ') : t('Managed by system')]]);
    const list = group(node, t('Interfaces'));
    if (!data.interfaces.length) el('p', t('No network interfaces found.'), list).className = 'empty-state';
    data.interfaces.forEach(iface => {
      const row = el('div', null, list); row.className = 'service-row';
      const left = el('div', null, row);
      el('strong', iface.name, left);
      el('small', iface.addresses.length ? iface.addresses.join(', ') : (iface.mac || '—'), left).className = 'group-description';
      statusBadge(row, iface.state === 'up' ? 'running' : iface.state === 'down' ? 'stopped' : 'warning');
    });
    const test = group(node, t('Connectivity test'), t('Opens brief TCP connections to the gateway, name resolution and the internet.'));
    const results = el('div', null, test); results.className = 'local-result'; results.setAttribute('role', 'status');
    button(actions(test), t('Test connectivity'), async () => {
      results.textContent = t('Testing…');
      const data = await api('/api/v1/recovery/network-test', {});
      results.replaceChildren();
      const labels = { gateway: t('Gateway'), 'dns-server': t('DNS server'), resolver: t('Name resolution'), internet: t('Internet') };
      data.checks.forEach(check => {
        const row = el('div', null, results); row.className = 'check-row';
        el('span', labels[check.id] || check.id, row);
        statusBadge(row, check.ok ? 'running' : 'error');
      });
      if (!data.checks.length) results.textContent = t('No checks possible.');
    });
  }
  async function logs() {
    const node = host('logs-host');
    const controls = el('div', null, node); controls.className = 'app-actions';
    const source = el('select', null, controls); source.className = 'secondary'; source.setAttribute('aria-label', t('Log source'));
    [['platform', t('Platform')], ['system', t('System')]].forEach(([value, label]) => { const option = el('option', label, source); option.value = value; });
    const level = el('select', null, controls); level.className = 'secondary'; level.setAttribute('aria-label', t('Level'));
    [['', t('All levels')], ['warning', t('Warnings + errors')], ['error', t('Errors only')]].forEach(([value, label]) => { const option = el('option', label, level); option.value = value; });
    const listing = el('div', null, node); listing.className = 'log-view';
    const pager = el('div', null, node); pager.className = 'pager';
    async function load(offset) {
      const query = new URLSearchParams({ source: source.value, limit: '150', offset: String(Math.max(0, offset)) });
      if (level.value) query.set('level', level.value);
      source.disabled = level.disabled = true;
      try {
        const data = await api(`/api/v1/recovery/logs?${query}`);
        listing.replaceChildren(); pager.replaceChildren();
        if (!data.available) el('p', t('Logs are unavailable on this device or access was denied.'), listing).className = 'inline-error';
        else if (!data.entries.length) el('p', t('No matching entries.'), listing).className = 'empty-state';
        else data.entries.forEach(entry => { const line = el('div', null, listing); line.className = 'log-line'; el('time', entry.at || '—', line); el('span', entry.message, line).className = 'log-message'; });
        if (offset > 0) button(pager, t('Newer'), () => load(offset - 150));
        if (data.available && data.entries.length === 150) button(pager, t('Older'), () => load(offset + 150));
      } finally { source.disabled = level.disabled = false; }
    }
    source.addEventListener('change', () => run(() => load(0)));
    level.addEventListener('change', () => run(() => load(0)));
    await load(0);
  }
  async function crashes() {
    const node = host('crashes-host');
    const data = await api('/api/v1/recovery/crashes');
    if (!data.apps.length) { el('p', t('No crashed or unresponsive apps in the activity log.'), node).className = 'empty-state'; return; }
    const catalog = await api('/api/v1/shell/apps').catch(() => ({ apps: [] }));
    const byId = new Map(catalog.apps.map(app => [app.id, app]));
    data.apps.forEach(entry => {
      const app = byId.get(entry.appId);
      const disabled = data.disabled.includes(entry.appId);
      const row = el('div', null, node); row.className = 'application-row';
      el('strong', app ? app.title : entry.appId, row);
      el('p', `${entry.appId} · ${entry.crashes} ${t('crashes')} · ${entry.unresponsive} ${t('unresponsive')}${entry.launchFailed ? ` · ${entry.launchFailed} ${t('launch failures')}` : ''}${disabled ? ` · ${t('disabled')}` : ''}`, row);
      if (entry.last) el('p', `${t('Last')}: ${new Date(entry.last * 1000).toLocaleString()}`, row);
      const commands = actions(row);
      if (app) {
        button(commands, t('Restart'), async () => {
          await api('/api/v1/recovery/restart', { target: 'app', appId: app.id, installationId: app.installationId });
          notice.textContent = `${app.title}: ${t('restarted')}`;
        });
        button(commands, disabled ? t('Enable') : t('Disable'), async () => {
          if (!disabled && !await confirmAction(`${app.title}: ${t('disable')}?`, t('The app will be stopped and no longer restarted automatically.'))) return;
          await api('/api/v1/recovery/quarantine', { appId: app.id, installationId: app.installationId, disabled: !disabled });
          notice.textContent = `${app.title}: ${disabled ? t('enabled') : t('disabled')}`;
          await loadWindow('crashes');
        });
      }
    });
  }
  async function boot() {
    const node = host('boot-host');
    const data = await api('/api/v1/recovery/boot');
    const overview = group(node, t('Boot'));
    const hours = data.uptime == null ? '—' : `${Math.floor(data.uptime / 3600)} h ${Math.floor((data.uptime % 3600) / 60)} min`;
    facts(overview, [[t('Kernel'), data.kernel || '—'], [t('Uptime'), hours]]);
    const failed = group(node, t('Failed services'));
    if (!data.failedUnits.length) el('p', t('No failed services.'), failed).className = 'empty-state';
    data.failedUnits.forEach(unit => { const row = el('div', null, failed); row.className = 'service-row'; el('span', unit.unit, row); statusBadge(row, 'error'); });
    const errors = group(node, t('Errors this boot'));
    if (!data.errors.length) el('p', t('No error logs available, or no errors.'), errors).className = 'empty-state';
    else { const view = el('div', null, errors); view.className = 'log-view'; data.errors.forEach(message => { const line = el('div', null, view); line.className = 'log-line'; el('span', message, line).className = 'log-message'; }); }
  }
  async function system() {
    const node = host('system-host');
    const info = await api('/api/v1/recovery/system');
    const release = info.osRelease || {};
    const overview = group(node, t('System status'));
    facts(overview, [[t('System'), release.PRETTY_NAME || release.NAME || 'rumahl OS'], [t('Version'), release.VERSION || release.BUILD_ID || '—'], [t('Build'), info.build || t('unknown')], [t('Kernel'), info.kernel || '—']]);
    const status = await hostState(node);
    const maintenance = group(node, t('Maintenance'), t('System changes require host permission and restart the device.'));
    if (!status || status.busy || status.restorePending) {
      el('p', t('Maintenance is currently unavailable or the device is busy.'), maintenance).className = 'group-description';
      return;
    }
    button(actions(maintenance), t('Roll back to previous build'), () => maintenance_row(maintenance, { action: 'rollback' }, t('Perform rollback?'), t('The device reboots using an earlier system state. Requires an earlier build to be present.')));
    const reset = group(node, t('Factory reset / reinstall'), t('Erases accounts, personal data and installed apps. Not reversible.'));
    button(actions(reset), t('Factory reset…'), () => maintenance_row(reset, { action: 'reset' }, t('Perform factory reset?'), t('All accounts, personal data and apps are erased and the device reboots. This cannot be undone.')));
    button(actions(reset), t('Reinstall…'), () => maintenance_row(reset, { action: 'reinstall' }, t('Reinstall the system?'), t('All data is erased and the device returns to its factory state. This cannot be undone.')));
    const power = group(node, t('Power off'), t('Safely powers the entire device off. It must be switched back on to continue.'));
    button(actions(power), t('Power off device'), () => maintenance_row(power, { action: 'poweroff' }, t('Power off the device?'), t('All running sessions and apps will stop. The device must then be switched on again manually.')));
  }
  async function maintenance_row(parent, request, title, description) {
    return maintenance(parent, request, title, description);
  }
  // Auto-redirect handover: a calm, self-diagnosing screen. A restart or
  // power-off appears only when the server signalled it (a WebSocket lifecycle
  // event, remembered in localStorage). Otherwise the screen tells "this device
  // is offline" apart from "rumahl OS is unreachable" and waits patiently.
  function handover(params) {
    const forced = params.get('state');
    const reason = params.get('reason');
    const detail = params.get('detail');
    let lifecycleState = null;
    try {
      const stored = JSON.parse(localStorage.getItem('rumahl.lifecycle') || 'null');
      if (stored && Date.now() - stored.at < 600000) lifecycleState = stored.state;
    } catch (error) { /* ignore */ }
    if (reason === 'poweroff') lifecycleState = 'shutting-down';
    else if (reason === 'restart' || reason === 'reboot') lifecycleState = 'restarting';
    const path = 'M 9.47163 471.21342 Q 0 471.21342 0 461.74179 V 203.63996 C 0 80.50881 80.50883 0 201.27207 0 H 241.52649 Q 250.99811 0 250.99811 9.47163 V 94.71626 Q 250.99811 104.18789 241.52649 104.18789 H 210.74370 C 144.44231 104.18789 101.81999 144.44230 101.81999 210.74369 V 461.74179 Q 101.81999 471.21342 92.34836 471.21342 Z';
    const style = el('style', null, document.head);
    style.textContent = '.hand{position:fixed;inset:0;z-index:2147483000;background:#000;color:#fff;display:grid;place-items:center;padding:32px}'
      + '.hand__mark{position:absolute;top:24px;right:28px;width:24px;height:auto;color:#fff}'
      + '.hand__center{max-width:440px;display:grid;gap:16px;justify-items:center;text-align:center}'
      + '.hand__spinner{width:30px;height:30px;border-radius:50%;border:2px solid #ffffff24;border-top-color:#fff;animation:handspin .8s linear infinite}'
      + '@keyframes handspin{to{transform:rotate(360deg)}}'
      + '.hand h1{margin:0;font-size:21px;font-weight:600;letter-spacing:-.01em}'
      + '.hand p{margin:0;color:#9aa2ae;font-size:14px;line-height:1.6}'
      + '.hand__tips{list-style:none;margin:2px 0 0;padding:0;display:grid;gap:8px;width:100%;text-align:left}'
      + '.hand__tips li{padding:10px 12px;border:1px solid #ffffff1a;border-radius:10px;background:#ffffff0a;color:#c3cad6;font-size:13px}'
      + '.hand__tips[hidden]{display:none}'
      + '.hand__actions{display:flex;gap:10px;flex-wrap:wrap;justify-content:center;margin-top:4px}'
      + '.hand button{border:1px solid #ffffff26;background:#ffffff10;color:#fff;border-radius:10px;padding:9px 16px;font:inherit;cursor:pointer}'
      + '.hand button.primary{background:#fff;color:#000;border-color:#fff}';
    const overlay = el('div', null, document.body); overlay.className = 'hand';
    overlay.setAttribute('role', 'status'); overlay.setAttribute('aria-live', 'polite');
    overlay.innerHTML = `<svg class="hand__mark" viewBox="0 0 251 472" fill="currentColor" aria-hidden="true"><path d="${path}"/></svg><div class="hand__center"><div class="hand__spinner"></div><h1></h1><p></p><ul class="hand__tips" hidden></ul><div class="hand__actions"></div></div>`;
    const spinner = overlay.querySelector('.hand__spinner');
    const title = overlay.querySelector('h1');
    const description = overlay.querySelector('p');
    const tips = overlay.querySelector('.hand__tips');
    const actionRow = overlay.querySelector('.hand__actions');
    const start = Date.now();
    let online = navigator.onLine !== false;
    let checking = false;
    let connectivity = 'unknown';
    let last = '';
    // The worker can probe the network; the page cannot (its CSP is connect-src
    // 'self'). This separates "this device is offline" from "rumahl OS offline".
    function classifyConnectivity() {
      return new Promise(resolve => {
        if (!online) return resolve('device');
        if (!navigator.serviceWorker || !navigator.serviceWorker.controller) return resolve('unknown');
        const channel = new MessageChannel();
        const timer = setTimeout(() => resolve('unknown'), 4000);
        channel.port1.onmessage = event => { clearTimeout(timer); resolve((event.data && event.data.connectivity) || 'unknown'); };
        navigator.serviceWorker.controller.postMessage({ type: 'RUMAHL_CONNECTIVITY' }, [channel.port2]);
      });
    }
    function phase() {
      if (forced) return forced === 'offline' || forced === 'device' ? 'device' : forced === 'error' || forced === 'server' ? 'unreachable' : forced === 'reboot' || forced === 'restart' ? 'restarting' : forced;
      if (!online || connectivity === 'device') return 'device';
      if (lifecycleState === 'shutting-down') return 'poweroff';
      if (lifecycleState === 'restarting') return 'restarting';
      const elapsed = Date.now() - start;
      if (elapsed < 15000) return 'connecting';
      if (elapsed < 60000) return 'slow';
      return 'unreachable';
    }
    function copy(state) {
      if (state === 'device') return [t('This device is offline'), t('There is no network connection. rumahl OS cannot be reached this way right now.')];
      if (state === 'connecting') return [t('Connecting…'), t('One moment while we connect you to rumahl OS.')];
      if (state === 'slow') return [t('This is taking a little longer…'), t('The system is not responding yet. We keep trying.')];
      if (state === 'restarting') return [t('rumahl is restarting…'), t('The device is restarting. This usually takes under a minute.')];
      if (state === 'poweroff') return [t('rumahl OS has shut down'), t('The device is powered off. Turn it back on to continue.')];
      return [t('rumahl OS is not reachable'), connectivity === 'server' ? t('Your connection seems fine, but the device itself is not responding.') : (detail && detail.trim() ? detail.slice(0, 200) : t('The device is not responding even after a longer time.'))];
    }
    function tipsFor(state) {
      if (state === 'device') return [t("Check this device's Wi-Fi or the network cable."), t('Check that your router or switch is powered on.'), t('Test another device or a mobile connection.')];
      if (state === 'connecting' || state === 'slow') return [t('Please give it a moment.'), t('A restart can take up to a minute.')];
      if (state === 'restarting') return [t('A restart in progress can take up to a minute.'), t('Do not cut power while the indicator is still active.')];
      if (state === 'poweroff') return [t('Turn the device back on.'), t('Check the power supply.')];
      return [t('Check that the rumahl device is powered on.'), t("Check the device's network connection (cable or Wi-Fi)."), t('Open the recovery tools to inspect the state.')];
    }
    function show(state) {
      if (state === last) return;
      last = state;
      const message = copy(state);
      title.textContent = message[0];
      description.textContent = message[1];
      spinner.hidden = state === 'device' || state === 'unreachable' || state === 'poweroff';
      tips.replaceChildren();
      const list = tipsFor(state);
      list.forEach(text => el('li', text, tips));
      tips.hidden = list.length === 0;
      actionRow.replaceChildren();
      const retry = el('button', t('Try again'), actionRow);
      retry.addEventListener('click', () => { last = ''; refreshConnectivity(); show(phase()); probe(); });
      if (state !== 'device' && state !== 'poweroff') {
        const tools = el('button', t('Open recovery tools'), actionRow);
        tools.className = 'primary';
        tools.addEventListener('click', () => overlay.remove());
      }
    }
    async function probe() {
      if (checking || document.hidden || forced || lifecycleState === 'shutting-down') return;
      checking = true;
      try {
        const response = await fetch('/', { credentials: 'same-origin', cache: 'no-store' });
        const type = response.headers.get('content-type') || '';
        if (response.ok && type.includes('text/html')) {
          try { localStorage.removeItem('rumahl.lifecycle'); } catch (error) { /* ignore */ }
          location.assign('/');
          return;
        }
      } catch (error) { /* keep waiting */ }
      finally { checking = false; }
    }
    function refreshConnectivity() { classifyConnectivity().then(value => { connectivity = value; }); }
    function tick() { show(phase()); probe(); }
    window.addEventListener('online', () => { online = true; refreshConnectivity(); tick(); });
    window.addEventListener('offline', () => { online = false; connectivity = 'device'; tick(); });
    document.addEventListener('visibilitychange', () => { if (!document.hidden) { refreshConnectivity(); tick(); } });
    refreshConnectivity();
    tick();
    setInterval(tick, 5000);
    setInterval(refreshConnectivity, 10000);
  }
  async function loadWindow(name) {
    const content = document.querySelector(`.page[data-page="${name}"]`);
    content.querySelector('.load-error')?.remove();
    content.setAttribute('aria-busy', 'true');
    try {
      if (name === 'control' || name === 'problems') await diagnose();
      if (name === 'settings') await settings();
      if (name === 'storage') await disks();
      if (name === 'diagnostics') await components();
      if (name === 'activity') await activity();
      if (name === 'backups') await backups();
      if (name === 'network') await network();
      if (name === 'logs') await logs();
      if (name === 'crashes') await crashes();
      if (name === 'boot') await boot();
      if (name === 'system') await system();
    } catch (error) {
      const message = el('p', error.message, content); message.className = 'load-error inline-error'; message.setAttribute('role', 'alert');
      throw error;
    } finally { content.setAttribute('aria-busy', 'false'); }
  }
  function open(name) {
    const entry = windows.get(name);
    if (!entry) return;
    page(name);
    if (!entry.loaded && !entry.loading) {
      entry.loading = run(async () => { await loadWindow(name); entry.loaded = true; });
      entry.loading.finally(() => { entry.loading = null; });
    }
  }
  document.querySelectorAll('[data-page]:not(.page)').forEach(node => node.addEventListener('click', event => {
    event.preventDefault(); open(node.dataset.page);
  }));
  const windows = new Map();
  const dock = document.getElementById('rre-dock');
  let topLayer = 1;
  function focusWindow(entry, moveFocus = true) {
    windows.forEach(item => item.frame.classList.toggle('is-focused', item === entry));
    entry.frame.style.zIndex = String(++topLayer);
    entry.dock.setAttribute('aria-expanded', 'true');
    if (moveFocus) entry.bar.focus({ preventScroll: true });
  }
  function fitWindow(entry) {
    if (entry.frame.hidden || entry.frame.classList.contains('is-maximized')) return;
    const area = main.getBoundingClientRect();
    const box = entry.frame.getBoundingClientRect();
    entry.frame.style.left = `${Math.max(0, Math.min(parseFloat(entry.frame.style.left) || 0, area.width - box.width))}px`;
    entry.frame.style.top = `${Math.max(0, Math.min(parseFloat(entry.frame.style.top) || 0, area.height - box.height))}px`;
  }
  function hideWindow(entry, close) {
    entry.frame.hidden = true;
    entry.dock.setAttribute('aria-expanded', 'false');
    if (close) { entry.opened = false; entry.dock.classList.remove('is-open'); }
    const remaining = [...windows.values()].filter(item => !item.frame.hidden).sort((a, b) => Number(b.frame.style.zIndex) - Number(a.frame.style.zIndex));
    if (remaining.length) focusWindow(remaining[0]); else entry.dock.focus();
  }
  function maximize(entry) {
    entry.frame.classList.toggle('is-maximized');
    entry.maxButton.setAttribute('aria-pressed', String(entry.frame.classList.contains('is-maximized')));
    fitWindow(entry);
  }
  function windowButton(parent, title, text, action) {
    const control = el('button', text, parent); control.type = 'button';
    control.title = title; control.setAttribute('aria-label', title);
    control.addEventListener('click', action); return control;
  }
  const paths = {
    backups: 'M3 7h18v14H3z M6 3h12v4 M8 12h8 M12 12v5 M9 15l3 3 3-3',
    activity: 'M5 3h14v18H5z M8 7h8 M8 11h8 M8 15h5',
    control: 'M3 4h18v12H3z M8 21h8 M12 16v5',
    problems: 'M2 12h4l3-8 6 16 3-8h4',
    diagnostics: 'M4 5h16v5H4z M4 14h16v5H4z M7 7.5h.01 M7 16.5h.01',
    storage: 'M5 4h14l2 12v4H3v-4z M3 16h18 M16 18h2',
    settings: 'M4 6h7 M15 6h5 M4 12h2 M10 12h10 M4 18h10 M18 18h2 M11 3v6 M6 9v6 M14 15v6',
    network: 'M5 12.5a10 10 0 0114 0 M8.5 16a5 5 0 017 0 M12 19.5h.01',
    logs: 'M6 3h9l4 4v14H6z M14 3v5h5 M9 12h6 M9 16h6',
    crashes: 'M12 4l9 16H3z M12 10v4 M12 17h.01',
    boot: 'M12 4v8 M7.5 6.8a7 7 0 109 0',
    system: 'M8 8h8v8H8z M4 9h2 M4 13h2 M18 9h2 M18 13h2 M9 4v2 M13 4v2 M9 18v2 M13 18v2'
  };
  document.querySelectorAll('.page').forEach((content, index) => {
    const name = content.dataset.page;
    const title = content.querySelector('h1').textContent;
    const frame = el('section', null, main); frame.className = 'rre-window'; frame.hidden = true;
    frame.id = `window-${name}`; frame.setAttribute('role', 'region');
    frame.setAttribute('aria-labelledby', `window-title-${name}`);
    frame.style.left = `${40 + index * 28}px`; frame.style.top = `${24 + index * 24}px`;
    const bar = el('div', null, frame); bar.className = 'window-bar'; bar.tabIndex = 0;
    bar.setAttribute('aria-label', `${title} — ${t('Move with arrow keys')}`);
    const heading = el('strong', title, bar); heading.id = `window-title-${name}`;
    const controls = el('div', null, bar); controls.className = 'window-controls';
    const entry = { frame, bar, opened: false, loaded: false, loading: null };
    windowButton(controls, t('Minimize'), '−', () => hideWindow(entry, false));
    entry.maxButton = windowButton(controls, t('Maximize / restore'), '□', () => maximize(entry));
    entry.maxButton.setAttribute('aria-pressed', 'false');
    windowButton(controls, t('Close'), '×', () => hideWindow(entry, true));
    const toolbar = el('div', null, frame); toolbar.className = 'window-toolbar';
    el('span', 'RRE', toolbar);
    button(toolbar, t('Refresh'), () => loadWindow(name));
    content.hidden = false; frame.append(content);
    const link = el('button', null, dock); link.type = 'button'; link.className = 'dock-item';
    link.setAttribute('aria-label', title); link.setAttribute('aria-controls', frame.id); link.setAttribute('aria-expanded', 'false');
    const icon = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    icon.setAttribute('viewBox', '0 0 24 24'); icon.setAttribute('aria-hidden', 'true');
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path'); path.setAttribute('d', paths[name]); icon.append(path);
    const tile = el('span', null, link); tile.className = `dock-icon icon-${name}`; tile.append(icon);
    el('span', title, link).className = 'dock-label';
    entry.dock = link; windows.set(name, entry);
    link.addEventListener('click', () => open(name));
    frame.addEventListener('pointerdown', () => focusWindow(entry, false));
    frame.addEventListener('focusin', () => focusWindow(entry, false));
    bar.addEventListener('dblclick', event => { if (!event.target.closest('button')) maximize(entry); });
    let drag = null;
    bar.addEventListener('pointerdown', event => {
      if (event.button !== 0 || event.target.closest('button') || frame.classList.contains('is-maximized') || matchMedia('(max-width: 650px)').matches) return;
      drag = { x: event.clientX, y: event.clientY, left: parseFloat(frame.style.left), top: parseFloat(frame.style.top) };
      bar.setPointerCapture(event.pointerId); event.preventDefault(); focusWindow(entry);
    });
    bar.addEventListener('pointermove', event => {
      if (!drag) return;
      frame.style.left = `${drag.left + event.clientX - drag.x}px`;
      frame.style.top = `${drag.top + event.clientY - drag.y}px`; fitWindow(entry);
    });
    bar.addEventListener('pointerup', () => { drag = null; });
    bar.addEventListener('pointercancel', () => { drag = null; });
    bar.addEventListener('lostpointercapture', () => { drag = null; });
    bar.addEventListener('keydown', event => {
      if (event.target !== bar || !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key) || frame.classList.contains('is-maximized')) return;
      event.preventDefault();
      const step = event.shiftKey ? 40 : 10;
      frame.style.left = `${parseFloat(frame.style.left) + (event.key === 'ArrowLeft' ? -step : event.key === 'ArrowRight' ? step : 0)}px`;
      frame.style.top = `${parseFloat(frame.style.top) + (event.key === 'ArrowUp' ? -step : event.key === 'ArrowDown' ? step : 0)}px`; fitWindow(entry);
    });
  });
  // Exit belongs to the RRE desktop: same dock vocabulary as the tools.
  (function () {
    const item = el('button', null, dock); item.type = 'button'; item.className = 'dock-item';
    item.setAttribute('aria-label', t('Exit'));
    const icon = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    icon.setAttribute('viewBox', '0 0 24 24'); icon.setAttribute('aria-hidden', 'true');
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', 'M14 4h5v16h-5 M11 8l-4 4 4 4 M7 12h9'); icon.append(path);
    const tile = el('span', null, item); tile.className = 'dock-icon icon-exit'; tile.append(icon);
    el('span', t('Exit'), item).className = 'dock-label';
    item.addEventListener('click', () => { location.href = '/'; });
  })();
  window.addEventListener('resize', () => windows.forEach(fitWindow));
  dock.addEventListener('keydown', event => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    const buttons = [...dock.querySelectorAll('button')];
    const index = buttons.indexOf(document.activeElement); if (index < 0) return;
    event.preventDefault();
    buttons[event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + buttons.length) % buttons.length].focus();
  });
  const search = new URLSearchParams(location.search);
  if (search.has('reason') || search.has('state')) handover(search);
  // Keep the desktop empty. Initial read-only checks fill the tools in the background.
  run(async () => { await diagnose(); windows.get('control').loaded = true; windows.get('problems').loaded = true; notice.textContent = t('Checks complete'); });
})();
