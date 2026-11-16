import { useEffect, useId, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import { useTranslation } from 'react-i18next';
import { Check, Copy, Download, RotateCcw, Trash2, Upload } from 'lucide-react';
import { BUILTIN_THEMES, THEME_COLOR_KEYS, createCustomTheme, resolveTheme, validateAppearance } from '../../lib/theme';
import type { AppearanceSettings, ThemeDefinition } from '../../lib/theme';
import { Select } from '../../components/Select';
import './appearance-editor.css';

interface Props {
  value: AppearanceSettings;
  onChange: (value: AppearanceSettings) => Promise<void>;
  onPreview: (value: AppearanceSettings | null) => void;
  onImport?: () => Promise<ThemeDefinition | null>;
  onExport?: (theme: ThemeDefinition) => Promise<void>;
}

export function AppearanceEditor({ value, onChange, onPreview, onImport, onExport }: Props) {
  const { t } = useTranslation();
  const id = useId();
  const [draft, setDraft] = useState<AppearanceSettings>(() => structuredClone(value));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const preview = useRef(onPreview);
  preview.current = onPreview;
  const savedKey = JSON.stringify(value);
  const draftKey = JSON.stringify(draft);
  const changed = draftKey !== savedKey;
  const selected = resolveTheme(draft);
  const custom = draft.customThemes.some(theme => theme.id === selected.id);
  let valid = true;
  try { validateAppearance(draft); } catch { valid = false; }

  // Playback snapshots may recreate value, but must not discard unfinished edits.
  useEffect(() => { setDraft(JSON.parse(savedKey) as AppearanceSettings); setError(null); }, [savedKey]);
  useEffect(() => {
    if (!changed) preview.current(null);
    else if (valid) preview.current(draft);
  }, [draft, changed, valid]);
  useEffect(() => () => preview.current(null), []);

  const update = (changes: Partial<ThemeDefinition>) => {
    if (!custom) return;
    setDraft(current => ({ ...current, customThemes: current.customThemes.map(theme => theme.id === current.activeThemeId ? { ...theme, ...changes } : theme) }));
    setError(null);
  };
  const duplicate = () => {
    try {
      const theme = createCustomTheme(selected, t('appearance.copyName', { name: custom ? selected.name : t(`appearance.presets.${selected.id}`) }).slice(0, 80), draft.customThemes);
      setDraft(current => ({ activeThemeId: theme.id, customThemes: [...current.customThemes, theme] }));
      setError(null);
    } catch { setError(t('appearance.themeLimit')); }
  };
  const reset = () => {
    const base = BUILTIN_THEMES.find(theme => theme.id === selected.base)!;
    update({ ...structuredClone(base), id: selected.id, name: selected.name });
  };
  const save = async () => {
    setBusy(true); setError(null);
    try { await onChange(validateAppearance(draft)); preview.current(null); }
    catch { setError(t('appearance.saveFailed')); }
    finally { setBusy(false); }
  };
  const importTheme = async () => {
    if (!onImport) return;
    setBusy(true); setError(null);
    try {
      const incoming = await onImport();
      if (incoming) {
        const theme = createCustomTheme(incoming, incoming.name, draft.customThemes);
        setDraft(current => ({ activeThemeId: theme.id, customThemes: [...current.customThemes, theme] }));
      }
    } catch { setError(t('appearance.importFailed')); }
    finally { setBusy(false); }
  };
  const exportTheme = async () => {
    if (!onExport) return;
    setBusy(true); setError(null);
    try { await onExport(selected); }
    catch { setError(t('appearance.exportFailed')); }
    finally { setBusy(false); }
  };
  const themeName = (theme: ThemeDefinition) => BUILTIN_THEMES.some(builtin => builtin.id === theme.id) ? t(`appearance.presets.${theme.id}`) : theme.name;

  return <div className="appearance-editor">
    <div className="appearance-intro"><p>{t('appearance.previewShort')}</p>
      <div className="appearance-file-actions" data-setting-id="appearance-files" tabIndex={-1}>
        {onImport && <button type="button" className="button secondary" disabled={busy || draft.customThemes.length >= 32} onClick={() => void importTheme()}><Upload size={14} />{t('appearance.import')}</button>}
        {onExport && <button type="button" className="button secondary" disabled={busy || !valid} onClick={() => void exportTheme()}><Download size={14} />{t('appearance.export')}</button>}
      </div>
    </div>
    {error && <p className="appearance-error" role="alert">{error}</p>}
    <div className="theme-presets" role="group" aria-label={t('appearance.choose')} data-setting-id="appearance-presets" tabIndex={-1}>
      {[...BUILTIN_THEMES, ...draft.customThemes].map(theme => <button type="button" key={theme.id} disabled={busy}
        className={`theme-preset ${theme.id === draft.activeThemeId ? 'selected' : ''}`} aria-pressed={theme.id === draft.activeThemeId}
        onClick={() => { setDraft(current => ({ ...current, activeThemeId: theme.id })); setError(null); }}>
        <span className="theme-thumbnail" aria-hidden="true" style={{ '--sample-bg': theme.colors.background, '--sample-sidebar': theme.colors.sidebar, '--sample-panel': theme.colors.panel, '--sample-accent': theme.colors.accent, '--sample-text': theme.colors.text } as CSSProperties}>
          <i className="sample-sidebar" /><i className="sample-heading" /><i className="sample-track sample-track-one" /><i className="sample-track sample-track-two" /><i className="sample-player"><i /></i>
        </span><span className="theme-preset-name">{themeName(theme)}{theme.id === draft.activeThemeId && <Check size={15} />}</span>
      </button>)}
    </div>
    <div className="theme-editor-heading" data-setting-id="appearance-customize" tabIndex={-1}><div><h3>{custom ? t('appearance.customize') : themeName(selected)}</h3><p>{custom ? t('appearance.previewHint') : t('appearance.builtinHint')}</p></div>
      <div className="theme-editor-actions">
        {custom && <button type="button" className="button quiet" disabled={busy} onClick={reset}><RotateCcw size={14} />{t('appearance.reset')}</button>}
        <button type="button" className="button secondary" disabled={busy || !valid || draft.customThemes.length >= 32} onClick={duplicate}><Copy size={14} />{t('appearance.duplicate')}</button>
      </div>
    </div>
    {custom && <div className="theme-name-row"><label htmlFor={`${id}-name`}>{t('appearance.name')}</label><input id={`${id}-name`} value={selected.name} maxLength={80} disabled={busy} onChange={event => update({ name: event.target.value })} />
      <button type="button" className="icon-button danger" disabled={busy} aria-label={t('appearance.delete')} title={t('appearance.delete')}
        onClick={() => setDraft(current => ({ activeThemeId: selected.base, customThemes: current.customThemes.filter(theme => theme.id !== selected.id) }))}><Trash2 size={16} /></button>
    </div>}
    <fieldset className="theme-fields" disabled={busy || !custom}>
      <legend>{t('appearance.colorsTitle')}</legend>
      <div className="theme-color-grid">{THEME_COLOR_KEYS.map(key => <div className="theme-color-field" key={key}>
        <label htmlFor={`${id}-${key}-text`}>{t(`appearance.colors.${key}`)}</label><div className="theme-color-inputs">
          <input type="color" aria-label={t('appearance.pickColor', { name: t(`appearance.colors.${key}`) })} value={/^#[0-9a-f]{6}$/i.test(selected.colors[key]) ? selected.colors[key] : '#000000'}
            onChange={event => update({ colors: { ...selected.colors, [key]: event.target.value.toUpperCase() } })} />
          <input id={`${id}-${key}-text`} type="text" value={selected.colors[key]} maxLength={7} spellCheck={false} autoComplete="off"
            aria-invalid={!/^#[0-9a-f]{6}$/i.test(selected.colors[key])} onChange={event => update({ colors: { ...selected.colors, [key]: event.target.value.toUpperCase() } })} />
        </div></div>)}</div>
    </fieldset>
    <fieldset className="theme-fields" disabled={busy || !custom} data-setting-id="appearance-typography" tabIndex={-1}><legend>{t('appearance.layoutTitle')}</legend>
      <div className="theme-layout-field"><label htmlFor={`${id}-font`}>{t('appearance.font')}</label>
        <Select id={`${id}-font`} value={selected.fontFamily} disabled={busy || !custom} label={t('appearance.font')}
          options={['system', 'sans', 'serif', 'mono'].map(font => ({ value: font, label: t(`appearance.fonts.${font}`) }))}
          onChange={fontFamily => update({ fontFamily: fontFamily as ThemeDefinition['fontFamily'] })} />
      </div>
      {(['fontScale', 'radius', 'spacing', 'shadows'] as const).map(key => {
        const limits = key === 'fontScale' ? [.85, 1.3] : key === 'radius' ? [0, 20] : key === 'spacing' ? [.8, 1.3] : [0, 1];
        const output = key === 'radius' ? `${selected[key]} px` : `${Math.round(selected[key] * 100)}%`;
        return <div className="theme-layout-field" key={key}><label htmlFor={`${id}-${key}`}>{t(`appearance.${key}`)}</label>
          <div className="theme-range-control"><input id={`${id}-${key}`} type="range" min={limits[0]} max={limits[1]} step="any" value={selected[key]} aria-valuetext={output}
            onChange={event => update({ [key]: Number(event.target.value) })} /><output htmlFor={`${id}-${key}`}>{output}</output></div>
        </div>;
      })}
    </fieldset>
    {!valid && <p className="appearance-error" role="alert">{t('appearance.invalid')}</p>}
    {changed && <div className="appearance-save-bar"><span role="status">{t('appearance.unsaved')}</span><div>
      <button type="button" className="button secondary" disabled={busy || !changed} onClick={() => { setDraft(structuredClone(value)); setError(null); preview.current(null); }}>{t('appearance.discard')}</button>
      <button type="button" className="button primary" disabled={busy || !changed || !valid} onClick={() => void save()}>{t('appearance.apply')}</button>
    </div></div>}
  </div>;
}
