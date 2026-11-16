import type { SVGProps } from 'react';
import type { RepeatMode } from '../../lib/backend/types';

interface Props extends Omit<SVGProps<SVGSVGElement>, 'children'> {
  mode: RepeatMode;
  size?: number;
}

/** Shared vector symbols stay legible in both the transport and its mode menu. */
export function RepeatModeIcon({ mode, size = 18, className = '', ...props }: Props) {
  return <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={2}
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
    focusable="false"
    className={`repeat-mode-icon repeat-mode-${mode} ${className}`}
    {...props}
  >
    {mode === 'off' ? <>
      <path d="M3 16h18m-4-4 4 4-4 4" />
      <path d="m10 6 2-1.5v7m-2 0h4" strokeWidth={1.8} />
    </> : <>
      <path d="m17 2 4 4-4 4M3 11v-1a4 4 0 0 1 4-4h14m-14 16-4-4 4-4m14-1v1a4 4 0 0 1-4 4H3" />
      {mode === 'all' ? <g strokeWidth={1.8}>
        <ellipse cx={11.5} cy={11.5} rx={2.8} ry={3.2} />
        <path d="m12.5 12.5 2.7 3" />
      </g> : <path d="m10 10 2-1.5v7m-2 0h4" strokeWidth={1.8} />}
    </>}
  </svg>;
}
