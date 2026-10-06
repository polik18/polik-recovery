'use strict';

const byId = (id) => document.getElementById(id);

function activateOnKeyboard(element, callback) {
  element.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      callback();
    }
  });
}

function switchTab(button) {
  const tabId = button.dataset.tab;
  document.querySelectorAll('.tab-pane').forEach((pane) => {
    const active = pane.id === tabId;
    pane.classList.toggle('active', active);
    pane.hidden = !active;
  });
  document.querySelectorAll('.tab-btn').forEach((candidate) => {
    const active = candidate === button;
    candidate.classList.toggle('active', active);
    candidate.setAttribute('aria-selected', String(active));
    candidate.tabIndex = active ? 0 : -1;
  });
}

function changeMode(mode) {
  if (!['all', 'novice', 'expert'].includes(mode)) {
    mode = 'all';
  }

  document.body.classList.remove('mode-all', 'mode-novice', 'mode-expert');
  document.body.classList.add(`mode-${mode}`);

  document.querySelectorAll('.selector-btn').forEach((button) => {
    const active = button.dataset.mode === mode;
    button.classList.toggle('active', active);
    button.setAttribute('aria-pressed', String(active));
  });

  try {
    localStorage.setItem('polik-recovery-mode', mode);
  } catch (_error) {
    // The preference is optional when storage is unavailable.
  }

  const sidebar = document.querySelector('.sidebar');
  if (sidebar) {
    sidebar.hidden = mode === 'novice';
  }
}

const mockupData = {
  mft: {
    name: '/$MFT',
    status: '已分配 (Allocated)',
    statusColor: '#00FF87',
    size: '16.0 KB',
    hash: '系統中繼檔 (不建議復原)',
    btnColor: '#6B7280',
    btnText: '🔒 系統保護檔案'
  },
  report: {
    name: 'report.txt',
    status: '已刪除 (Deleted)',
    statusColor: '#FF416C',
    size: '42 位元組',
    hash: 'SHA-256 示意',
    btnColor: '#10B981',
    btnText: '💾 模擬復原'
  },
  doc: {
    name: 'document_old.doc',
    status: '已刪除 (Deleted)',
    statusColor: '#FF416C',
    size: '15.5 KB',
    hash: 'SHA-256 示意',
    btnColor: '#10B981',
    btnText: '💾 模擬復原'
  }
};

function selectMockupItem(key) {
  const data = mockupData[key];
  if (!data) {
    return;
  }

  document.querySelectorAll('.mockup-vfs-item').forEach((item) => {
    const active = item.dataset.mockupItem === key;
    item.classList.toggle('active', active);
    item.setAttribute('aria-pressed', String(active));
    const rect = item.querySelector('rect');
    rect.setAttribute('fill', active ? 'rgba(0, 242, 254, 0.1)' : 'transparent');
    if (active) {
      rect.setAttribute('stroke', 'rgba(0, 242, 254, 0.2)');
    } else {
      rect.removeAttribute('stroke');
    }
  });

  byId('mockup-det-name').textContent = data.name;
  const status = byId('mockup-det-status');
  status.textContent = data.status;
  status.setAttribute('fill', data.statusColor);
  byId('mockup-det-size').textContent = data.size;
  byId('mockup-det-hash').textContent = data.hash;
  byId('mockup-btn-rect').setAttribute('fill', data.btnColor);
  byId('mockup-btn-text').textContent = data.btnText;
}

function updateLog(message) {
  const terminal = byId('sim-terminal-log');
  const prompt = document.createElement('span');
  prompt.className = 'terminal-prompt';
  prompt.textContent = '>';
  terminal.replaceChildren(prompt, document.createTextNode(` ${message}`));
}

function simulateDelete() {
  const card = byId('card-report');
  card.classList.remove('sim-recovered', 'sim-overwritten');
  card.classList.add('sim-deleted');

  const badge = byId('badge-report');
  badge.className = 'card-status-badge status-deleted';
  badge.textContent = '已刪除 (Deleted)';

  const sector = byId('sector-2');
  sector.classList.remove('sim-overwritten');
  sector.classList.add('sim-source-retained');

  updateLog('模擬刪除完成：目錄卡已標示刪除，Sector 2 的示意內容仍存在。現在可執行步驟二。');
  byId('btn-sim-delete').disabled = true;
  byId('btn-sim-recover').disabled = false;
}

function simulateRecovery() {
  updateLog('正在執行介面模擬；本頁不會讀取真實磁碟或建立復原檔案。');
  window.setTimeout(() => {
    const card = byId('card-report');
    card.classList.remove('sim-deleted', 'sim-overwritten');
    card.classList.add('sim-recovered');

    const badge = byId('badge-report');
    badge.className = 'card-status-badge status-allocated';
    badge.textContent = '模擬救援 (Simulated)';

    updateLog('介面模擬完成。這不是實際復原結果；步驟三將示範來源資料遭覆寫的風險。');
    byId('btn-sim-recover').disabled = true;
    byId('btn-sim-overwrite').disabled = false;
  }, 600);
}

function simulateOverwrite() {
  byId('content-sector-2').textContent = '「新檔案內容：覆寫了舊資料。」';
  const sector = byId('sector-2');
  sector.className = 'disk-sector';
  sector.classList.add('sim-overwritten');

  const card = byId('card-report');
  card.classList.remove('sim-deleted', 'sim-recovered');
  card.classList.add('sim-overwritten');

  const badge = byId('badge-report');
  badge.className = 'card-status-badge status-deleted';
  badge.textContent = '已覆寫 (Overwritten)';
  updateLog('警告示意：來源區塊被新內容覆寫後，舊 bytes 通常無法再由軟體復原。');
  byId('btn-sim-overwrite').disabled = true;
}

function resetSimulation() {
  const card = byId('card-report');
  card.classList.remove('sim-deleted', 'sim-recovered', 'sim-overwritten');

  const badge = byId('badge-report');
  badge.className = 'card-status-badge status-allocated';
  badge.textContent = '已分配 (Allocated)';

  const sector = byId('sector-2');
  sector.className = 'disk-sector sector-occupied';
  byId('content-sector-2').textContent = '「報告內容: Rust 救援引擎完成測試。」';

  byId('btn-sim-delete').disabled = false;
  byId('btn-sim-recover').disabled = true;
  byId('btn-sim-overwrite').disabled = true;
  updateLog('系統重置完畢。請點擊步驟一開始模擬。');
}

document.addEventListener('DOMContentLoaded', () => {
  let savedMode = 'all';
  try {
    savedMode = localStorage.getItem('polik-recovery-mode') || 'all';
  } catch (_error) {
    // Default mode remains available without storage access.
  }
  changeMode(savedMode);

  document.querySelectorAll('.selector-btn').forEach((button) => {
    button.addEventListener('click', () => changeMode(button.dataset.mode));
  });

  const tabButtons = [...document.querySelectorAll('.tab-btn')];
  tabButtons.forEach((button, index) => {
    button.addEventListener('click', () => switchTab(button));
    button.addEventListener('keydown', (event) => {
      let targetIndex = null;
      if (event.key === 'ArrowRight') targetIndex = (index + 1) % tabButtons.length;
      if (event.key === 'ArrowLeft') targetIndex = (index - 1 + tabButtons.length) % tabButtons.length;
      if (event.key === 'Home') targetIndex = 0;
      if (event.key === 'End') targetIndex = tabButtons.length - 1;
      if (targetIndex !== null) {
        event.preventDefault();
        tabButtons[targetIndex].focus();
        switchTab(tabButtons[targetIndex]);
      }
    });
  });

  document.querySelectorAll('[data-mockup-item]').forEach((item) => {
    const select = () => selectMockupItem(item.dataset.mockupItem);
    item.addEventListener('click', select);
    activateOnKeyboard(item, select);
  });

  const mockupAction = byId('mockup-recovery-action');
  const explainMockup = () => window.alert('桌面端介面模擬：本按鈕不會執行磁碟讀取或檔案復原。');
  mockupAction.addEventListener('click', explainMockup);
  activateOnKeyboard(mockupAction, explainMockup);

  byId('btn-sim-delete').addEventListener('click', simulateDelete);
  byId('btn-sim-recover').addEventListener('click', simulateRecovery);
  byId('btn-sim-overwrite').addEventListener('click', simulateOverwrite);
  byId('btn-sim-reset').addEventListener('click', resetSimulation);
});
