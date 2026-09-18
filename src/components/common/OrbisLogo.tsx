import React from 'react';

interface OrbisLogoProps {
  className?: string;
  showText?: boolean;
  size?: 'sm' | 'md' | 'lg';
}

export const OrbisLogo: React.FC<OrbisLogoProps> = ({
  className = '',
  showText = true,
  size = 'md',
}) => {
  if (size === 'lg') {
    return (
      <div className={`relative group cursor-default ${className}`}>
        <div className="absolute inset-0 rounded-full bg-primary-container/20 blur-md pointer-events-none" />
        <svg
          className="relative w-12 h-12"
          viewBox="0 0 48 48"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          {/* Outer Cyan Ring */}
          <circle cx="24" cy="24" r="17" stroke="#00E5FF" strokeWidth="4.5" />
          {/* Inner Cyan Core Dot */}
          <circle cx="24" cy="24" r="6" fill="#00E5FF" />
          {/* Neon Fuchsia Diagonal Slash */}
          <line
            x1="8"
            y1="40"
            x2="40"
            y2="8"
            stroke="#C77DFF"
            strokeWidth="4"
            strokeLinecap="round"
          />
        </svg>
      </div>
    );
  }

  return (
    <div className={`flex items-center gap-space-sm cursor-pointer select-none ${className}`}>
      <svg
        viewBox="0 0 32 32"
        width="24"
        height="24"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        className="shrink-0"
      >
        <circle cx="16" cy="16" r="11" stroke="#00E5FF" strokeWidth="2.5" />
        <circle cx="16" cy="16" r="4.5" fill="#00E5FF" />
        <path
          d="M6 26L26 6"
          stroke="#C77DFF"
          strokeWidth="2"
          strokeLinecap="round"
        />
      </svg>
      {showText && (
        <span className="font-headline-md text-headline-md tracking-wider text-text-primary uppercase font-bold">
          Orbis
        </span>
      )}
    </div>
  );
};
