'use strict';
// 画面はすべて textContent / value で組み立てる(サーバーから来た文字列を HTML として解釈しない)。
const DAYS = [
  ['MONDAY', '月'], ['TUESDAY', '火'], ['WEDNESDAY', '水'], ['THURSDAY', '木'],
  ['FRIDAY', '金'], ['SATURDAY', '土'], ['SUNDAY', '日'],
];
const $ = (id) => document.getElementById(id);
const el = (tag, props = {}, ...kids) => {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === 'class') e.className = v; else if (k === 'text') e.textContent = v;
    else if (k.startsWith('on')) e.addEventListener(k.slice(2), v); else e.setAttribute(k, v);
  }
  for (const c of kids) e.append(c);
  return e;
};

let S = null;            // /api/state の内容
let editingId = null;    // 編集中のアラームid
let numbers = { main: [], pre: [] }; // [[id, 番号], ...]

async function api(path, body) {
  const opt = body === undefined ? {} : {
    method: 'POST',
    headers: { 'content-type': 'application/json', 'x-maid-cafe': '1' },
    body: JSON.stringify(body),
  };
  const r = await fetch(path, opt);
  const j = await r.json().catch(() => ({}));
  if (!r.ok) throw new Error(j.error || `エラー (${r.status})`);
  return j;
}

const recText = (r) => {
  switch (r.kind) {
    case 'daily': return '毎日';
    case 'weekdays': return r.skip_holidays ? '平日(祝日を除く)' : '平日';
    case 'weekends': return '土日祝';
    case 'dow': return r.days.map((d) => DAYS.find((x) => x[0] === d)?.[1] ?? d).join('');
    case 'nth': return `第${r.nth === -1 ? '最終' : r.nth}${DAYS.find((x) => x[0] === r.day)?.[1] ?? ''}曜日`;
    case 'every': return `${r.interval}週ごと(${r.days.map((d) => DAYS.find((x) => x[0] === d)?.[1] ?? d).join('')})`;
    default: return '';
  }
};

function render() {
  $('ver').textContent = `v${S.version}`;
  $('next').textContent = S.next ? `次のアラーム: ${S.next.time} ${S.next.title}` : '次のアラームはありません';
  $('stop').hidden = !S.playing;
  const ul = $('alarms');
  ul.replaceChildren();
  $('empty').hidden = S.alarms.length > 0;
  for (const a of S.alarms) {
    const sub = [recText(a.recurrence), a.kind === 'SPEECH' ? (a.speech_sound ? '声+音' : '声のみ') : '音', a.voice === 'DEEP_MALE' ? '低い男性' : 'メイド風',
      a.pre_notice_minutes ? `${a.pre_notice_minutes}分前に予告` : '', a.harmony ? 'ハモり' : ''].filter(Boolean).join(' ・ ');
    const toggle = el('input', { type: 'checkbox', 'aria-label': '有効' });
    toggle.checked = a.enabled;
    toggle.addEventListener('change', () => act(() => api('/api/alarms/toggle', { id: a.id, enabled: toggle.checked })));
    ul.append(el('li', { class: 'alarm' + (a.enabled ? '' : ' off') },
      el('div', { class: 'time', text: a.time }),
      el('div', { class: 'body' }, el('div', { class: 'title', text: a.label }), el('div', { class: 'sub', text: sub })),
      toggle,
      el('button', { text: '編集', onclick: () => openEditor(a) }),
      el('button', { class: 'danger', text: '削除', onclick: () => { if (confirm(`「${a.label}」を削除しますか?`)) act(() => api('/api/alarms/delete', { id: a.id })); } }),
    ));
  }
}

async function act(fn) {
  try { await fn(); } catch (e) { alert(e.message); }
  await refresh();
}

async function refresh() {
  S = await api('/api/state');
  render();
}

// ---- 編集ダイアログ ----
function buildDow(container, selected) {
  container.replaceChildren();
  const wrap = el('div', { class: 'dow' });
  for (const [v, n] of DAYS) {
    const c = el('input', { type: 'checkbox', value: v });
    c.checked = selected.includes(v);
    wrap.append(el('label', {}, c, n));
  }
  container.append(wrap);
}
const pickedDays = () => [...$('rec-dow').querySelectorAll('input:checked')].map((c) => c.value);

function showRecFields() {
  const v = $('f-rec').value;
  $('rec-dow').hidden = !(v === 'dow' || v === 'every');
  $('rec-nth').hidden = v !== 'nth';
  $('rec-every').hidden = v !== 'every';
}
function showKindFields() {
  const speech = $('f-kind').checked;
  $('l-ss').hidden = !speech;
  $('l-sound').hidden = speech && !$('f-ss').checked; // 声だけなら、音の選択は要らない
  $('l-text').hidden = !speech;
}

function recurrenceFromForm() {
  switch ($('f-rec').value) {
    case 'daily': return { kind: 'daily' };
    case 'weekdays': return { kind: 'weekdays', skip_holidays: false };
    case 'weekdays_skip': return { kind: 'weekdays', skip_holidays: true };
    case 'weekends': return { kind: 'weekends' };
    case 'dow': return { kind: 'dow', days: pickedDays() };
    case 'nth': return { kind: 'nth', nth: parseInt($('f-nth').value, 10), day: $('f-nth-day').value };
    default: return { kind: 'every', interval: parseInt($('f-interval').value, 10) || 1, days: pickedDays(), anchor: $('f-anchor').value };
  }
}
function recurrenceToForm(r) {
  let sel = r.kind;
  if (r.kind === 'weekdays') sel = r.skip_holidays ? 'weekdays_skip' : 'weekdays';
  $('f-rec').value = sel;
  buildDow($('rec-dow'), r.days || []);
  if (r.kind === 'nth') { $('f-nth').value = String(r.nth); $('f-nth-day').value = r.day; }
  if (r.kind === 'every') { $('f-interval').value = r.interval; $('f-anchor').value = r.anchor; }
  showRecFields();
}

function renderPhrases(which, container) {
  container.replaceChildren();
  const cur = numbers[which];
  for (const p of S.phrases) {
    const entry = cur.find((x) => x[0] === p.id);
    const cb = el('input', { type: 'checkbox' });
    cb.checked = !!entry;
    const num = el('input', { type: 'number', min: '1', max: '99', class: 'n', 'aria-label': '喋る順の番号' });
    num.value = entry ? entry[1] : '';
    const edit = async (op, n) => {
      try {
        const r = await api('/api/phrases/edit', { numbers: numbers[which], op, id: p.id, n });
        numbers[which] = r.numbers;
      } catch (e) { $('form-error').textContent = e.message; }
      renderPhrases(which, container);
    };
    cb.addEventListener('change', () => edit(cb.checked ? 'add' : 'remove', 0));
    num.addEventListener('change', () => { const n = parseInt(num.value, 10); if (n >= 1) edit('assign', n); else renderPhrases(which, container); });
    container.append(el('div', { class: 'phrase' + (entry ? ' on' : '') }, cb, num, el('span', { text: p.display })));
  }
}

const ranksOf = (ids) => ids.map((id, i) => [id, i + 1]);
const orderedIds = (nums) => [...nums].sort((a, b) => a[1] - b[1]).map((x) => x[0]);

function openEditor(a) {
  editingId = a ? a.id : null;
  $('editor-title').textContent = a ? 'アラームを編集' : 'アラームを追加';
  $('form-error').textContent = '';
  const d = a || { label: '', time: '07:00', recurrence: { kind: 'daily' }, kind: 'SOUND', sound: 'chime', text: '', voice: 'MAID', phrases: [], pre_phrases: [], harmony: false, pre_notice_minutes: null };
  $('f-label').value = d.label;
  $('f-time').value = d.time;
  $('f-kind').checked = d.kind === 'SPEECH';
  $('f-ss').checked = !!d.speech_sound;
  $('f-sound').value = d.sound || 'chime';
  $('f-text').value = d.text;
  $('f-voice').value = d.voice;
  $('f-harmony').checked = d.harmony;
  $('f-pre').checked = d.pre_notice_minutes != null;
  $('f-pre-min').value = d.pre_notice_minutes ?? 30;
  $('f-anchor').value = d.recurrence.anchor || new Date().toISOString().slice(0, 10);
  recurrenceToForm(d.recurrence);
  showKindFields();
  numbers = { main: ranksOf(d.phrases), pre: ranksOf(d.pre_phrases) };
  renderPhrases('main', $('phrases'));
  renderPhrases('pre', $('pre-phrases'));
  syncPre();
  $('editor').showModal();
}
const syncPre = () => { $('pre-set').hidden = !$('f-pre').checked; $('f-pre-min').disabled = !$('f-pre').checked; };

function alarmFromForm() {
  return {
    id: editingId || `a${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`,
    label: $('f-label').value,
    time: $('f-time').value,
    recurrence: recurrenceFromForm(),
    kind: $('f-kind').checked ? 'SPEECH' : 'SOUND',
    speech_sound: $('f-kind').checked && $('f-ss').checked,
    sound: $('f-sound').value,
    text: $('f-text').value,
    voice: $('f-voice').value,
    enabled: editingId ? (S.alarms.find((x) => x.id === editingId)?.enabled ?? true) : true,
    phrases: orderedIds(numbers.main),
    pre_phrases: orderedIds(numbers.pre),
    harmony: $('f-harmony').checked,
    pre_notice_minutes: $('f-pre').checked ? parseInt($('f-pre-min').value, 10) : null,
  };
}

// ---- 設定 ----
function fillVoiceSelect(sel, current) {
  sel.replaceChildren(el('option', { value: '', text: '自動(日本語の音声を優先)' }));
  for (const v of S.voices) sel.append(el('option', { value: v.name, text: `${v.name} (${v.culture})` }));
  sel.value = current || '';
}
async function saveSettings(body) {
  $('settings-error').textContent = '';
  try { await api('/api/settings', body); await refresh(); } catch (e) { $('settings-error').textContent = e.message; }
}

function wire() {
  $('add').onclick = () => openEditor(null);
  $('stop').onclick = () => act(() => api('/api/stop', {}));
  $('f-rec').onchange = showRecFields;
  $('f-kind').onchange = showKindFields;
  $('f-ss').onchange = showKindFields;
  $('f-pre').onchange = syncPre;
  $('cancel').onclick = () => $('editor').close();
  $('form').addEventListener('submit', async (ev) => {
    ev.preventDefault();
    try { await api('/api/alarms', alarmFromForm()); $('editor').close(); await refresh(); } catch (e) { $('form-error').textContent = e.message; }
  });
  $('test').onclick = async () => {
    $('form-error').textContent = '';
    try { await api('/api/test', alarmFromForm()); $('form-error').textContent = '再生中… (止めるには、画面上部の「止める」)'; await refresh(); } catch (e) { $('form-error').textContent = e.message; }
  };
  $('open-settings').onclick = () => {
    fillVoiceSelect($('s-maid'), S.prefs.MAID);
    fillVoiceSelect($('s-male'), S.prefs.DEEP_MALE);
    $('s-auto').checked = S.autostart;
    $('voice-note').textContent = S.voices.some((v) => v.japanese) ? '' : '日本語の音声が見つかりません。Windowsの設定 → 時刻と言語 → 音声認識(言語とリージョン)で日本語を追加すると読み上げできます。';
    $('settings-error').textContent = '';
    $('settings').showModal();
  };
  $('settings-close').onclick = () => $('settings').close();
  $('s-maid').onchange = () => saveSettings({ voice_maid: $('s-maid').value });
  $('s-male').onchange = () => saveSettings({ voice_deep_male: $('s-male').value });
  $('s-auto').onchange = () => saveSettings({ autostart: $('s-auto').checked });
}

(async () => {
  try { S = await api('/api/state'); } catch (e) { $('next').textContent = e.message; return; }
  const sel = $('f-sound');
  for (const s of S.sounds) sel.append(el('option', { value: s.id, text: s.name }));
  for (const [v, n] of DAYS) $('f-nth-day').append(el('option', { value: v, text: `${n}曜日` }));
  wire();
  render();
  setInterval(() => refresh().catch(() => {}), 5000);
})();
