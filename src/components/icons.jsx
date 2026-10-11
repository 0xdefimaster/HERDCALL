import React from 'react';

function PixelSvg({ size, viewBox, title, className = '', children }) {
  const a11y = title ? { role: 'img', 'aria-label': title } : { 'aria-hidden': true };
  return (
    <svg width={size} height={size} viewBox={viewBox} shapeRendering="crispEdges" fill="currentColor" className={className} {...a11y}>
      {children}
    </svg>
  );
}

export function HLogo({ size = 28, title, className = 'text-hc-orange' }) {
  return (
    <PixelSvg size={size} viewBox="0 0 12 12" title={title} className={className}>
      <rect x="0" y="0" width="2" height="2" />
      <rect x="1" y="2" width="2" height="2" />
      <rect x="2" y="4" width="2" height="8" />
      <rect x="4" y="7" width="4" height="2" />
      <rect x="8" y="4" width="2" height="8" />
      <rect x="9" y="2" width="2" height="2" />
      <rect x="10" y="0" width="2" height="2" />
    </PixelSvg>
  );
}

export function ArrowIcon({ size = 18 }) {
  return (
    <PixelSvg size={size} viewBox="0 0 6 6">
      <rect x="0" y="2" width="4" height="2" />
      <rect x="3" y="1" width="1" height="4" />
      <rect x="4" y="2" width="1" height="2" />
      <rect x="2" y="0" width="1" height="1" />
      <rect x="2" y="5" width="1" height="1" />
    </PixelSvg>
  );
}

export function ChartIcon({ size = 32 }) {
  return (
    <PixelSvg size={size} viewBox="0 0 8 8" className="text-hc-orange">
      <rect x="0" y="7" width="8" height="1" />
      <rect x="1" y="4" width="1" height="3" />
      <rect x="3" y="2" width="1" height="5" />
      <rect x="5" y="5" width="1" height="2" />
      <rect x="7" y="0" width="1" height="7" />
    </PixelSvg>
  );
}

export function CycleIcon({ size = 32 }) {
  return (
    <PixelSvg size={size} viewBox="0 0 8 8" className="text-hc-orange">
      <rect x="2" y="0" width="4" height="1" />
      <rect x="1" y="1" width="1" height="1" />
      <rect x="6" y="1" width="1" height="1" />
      <rect x="0" y="2" width="1" height="4" />
      <rect x="7" y="2" width="1" height="4" />
      <rect x="1" y="6" width="1" height="1" />
      <rect x="6" y="6" width="1" height="1" />
      <rect x="2" y="7" width="4" height="1" />
      <rect x="3" y="2" width="2" height="1" />
      <rect x="3" y="3" width="1" height="2" />
      <rect x="3" y="5" width="2" height="1" />
    </PixelSvg>
  );
}

export function FlagIcon({ size = 32 }) {
  return (
    <PixelSvg size={size} viewBox="0 0 8 8" className="text-hc-orange">
      <rect x="0" y="0" width="1" height="8" />
      <rect x="1" y="0" width="5" height="1" />
      <rect x="5" y="1" width="2" height="1" />
      <rect x="6" y="2" width="2" height="1" />
      <rect x="5" y="3" width="2" height="1" />
      <rect x="1" y="4" width="5" height="1" />
    </PixelSvg>
  );
}

export function CheckIcon({ size = 20 }) {
  return (
    <PixelSvg size={size} viewBox="0 0 8 8" className="text-hc-orange shrink-0 mt-1">
      <rect x="0" y="4" width="1" height="1" />
      <rect x="1" y="5" width="1" height="1" />
      <rect x="2" y="6" width="1" height="1" />
      <rect x="3" y="5" width="1" height="1" />
      <rect x="4" y="4" width="1" height="1" />
      <rect x="5" y="3" width="1" height="1" />
      <rect x="6" y="2" width="1" height="1" />
      <rect x="7" y="1" width="1" height="1" />
    </PixelSvg>
  );
}

export function ImageIcon({ size = 30 }) {
  return (
    <PixelSvg size={size} viewBox="0 0 8 8" className="text-hc-muted">
      <rect x="0" y="0" width="8" height="1" />
      <rect x="0" y="7" width="8" height="1" />
      <rect x="0" y="1" width="1" height="6" />
      <rect x="7" y="1" width="1" height="6" />
      <rect x="2" y="2" width="1" height="1" />
      <rect x="1" y="6" width="2" height="1" />
      <rect x="3" y="5" width="1" height="1" />
      <rect x="4" y="4" width="1" height="2" />
      <rect x="5" y="5" width="2" height="1" />
    </PixelSvg>
  );
}
