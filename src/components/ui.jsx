import React from 'react';
import { Link } from 'react-router-dom';

const BUTTON_BASE =
  'inline-flex items-center justify-center gap-2 min-h-[44px] px-5 border-[3px] font-pixel font-bold text-[15px] no-underline transition-transform active:translate-x-[2px] active:translate-y-[2px] disabled:opacity-50 disabled:cursor-not-allowed';

const BUTTON_VARIANTS = {
  primary: 'bg-hc-orange text-hc-bg border-hc-bg shadow-px-cream hover:bg-hc-orange-hover hover:text-hc-bg',
  secondary: 'bg-transparent text-hc-cream border-hc-cream shadow-px-orange hover:text-hc-cream',
  ghost: 'bg-transparent text-hc-cream border-hc-cream font-normal text-[13px] hover:text-hc-cream',
};

export function PixelButton({ to, variant = 'primary', className = '', children, ...rest }) {
  const classes = `${BUTTON_BASE} ${BUTTON_VARIANTS[variant]} ${className}`;
  if (to) {
    return (
      <Link to={to} className={classes} {...rest}>
        {children}
      </Link>
    );
  }
  return (
    <button type="button" className={classes} {...rest}>
      {children}
    </button>
  );
}

export function Card({ as: Tag = 'section', highlight = false, className = '', children, ...rest }) {
  const border = highlight ? 'border-hc-orange shadow-px-navy' : 'border-hc-line';
  return (
    <Tag className={`bg-hc-surface border-[3px] ${border} p-5 ${className}`} {...rest}>
      {children}
    </Tag>
  );
}

export function CardTitle({ as: Tag = 'h2', className = '', children }) {
  return <Tag className={`font-pixel font-bold text-lg uppercase m-0 ${className}`}>{children}</Tag>;
}

export function Bar({ value, color = 'bg-hc-orange', className = 'h-5' }) {
  return (
    <div className={`bg-hc-bg border-[3px] border-hc-line box-border ${className}`}>
      <div className={`h-full ${color}`} style={{ width: `${value}%` }} />
    </div>
  );
}

export function Tag({ tone = 'navy', className = '', children }) {
  const tones = {
    navy: 'bg-hc-navy text-hc-navy-text',
    orange: 'bg-hc-orange text-hc-bg font-bold',
    outline: 'border-2 border-hc-line text-hc-body',
  };
  return <span className={`inline-block px-2.5 py-1 font-pixel text-xs ${tones[tone]} ${className}`}>{children}</span>;
}

export function Row({ label, children, className = '' }) {
  return (
    <div className={`flex justify-between gap-2 ${className}`}>
      <span className="text-hc-muted">{label}</span>
      <span>{children}</span>
    </div>
  );
}

export function Sparkline({ points, up, width = 96, height = 28 }) {
  const min = Math.min(...points);
  const max = Math.max(...points);
  const span = max - min || 1;
  const step = width / (points.length - 1);
  const path = points.map((p, i) => `${(i * step).toFixed(1)},${(height - 2 - ((p - min) / span) * (height - 4)).toFixed(1)}`).join(' ');
  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} aria-hidden="true" className="block">
      <polyline fill="none" stroke={up ? '#7EE081' : '#FF6B6B'} strokeWidth="2" strokeLinejoin="round" points={path} />
    </svg>
  );
}

export function Change({ value }) {
  if (!value) return <span className="text-hc-muted">—</span>;
  const up = value > 0;
  return (
    <span className={`font-terminal text-xl leading-none whitespace-nowrap ${up ? 'text-hc-up' : 'text-hc-down'}`}>
      {up ? '▲' : '▼'} {Math.abs(value).toFixed(1)}%
    </span>
  );
}

export function TokenAvatar({ initial, size = 'w-16 h-16 text-3xl' }) {
  return (
    <div className={`${size} shrink-0 bg-hc-navy border-[3px] border-hc-orange text-hc-orange flex items-center justify-center font-pixel font-bold`}>
      {initial}
    </div>
  );
}
