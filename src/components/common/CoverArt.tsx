/**
 * 游戏封面（离线安全的占位实现）
 *
 * 为什么不用外部图片：MVP 的 NFR 要求「除版本比对数据源外全部功能离线可用」（02 §5），
 * 而原前端实现引用的是外部图床链接 —— 离线即碎、且属于未授权使用第三方素材。
 *
 * 因此这里用「设计系统内的渐变 + 字形」生成占位封面，并为将来的官方素材预留 `src` 通道。
 * 官方封面素材需要先解决权利问题（00 §8.6 License 治理口径同样适用于美术资源），
 * 未解决前一律走占位，不引入热链。
 */
import React from 'react';
import type { GameId } from '../../api/types';

/** 每款游戏一个稳定色调（暗底 + 低饱和双色渐变，不与主强调色争夺注意力） */
const GRADIENT: Record<GameId, string> = {
  'wuthering-waves': 'from-[#0d1416] via-[#101b1f] to-[#0b0f12]',
  'genshin-impact': 'from-[#141019] via-[#1a1322] to-[#0d0b12]',
  'honkai-star-rail': 'from-[#101418] via-[#141b23] to-[#0b0e12]',
  'zenless-zone-zero': 'from-[#161016] via-[#1d1420] to-[#0e0b0e]',
  'arknights-endfield': 'from-[#0f1313] via-[#131b19] to-[#0a0d0d]',
};

/** 占位水印字形（避免使用官方 LOGO，仅用游戏名首字） */
const GLYPH: Record<GameId, string> = {
  'wuthering-waves': '鸣',
  'genshin-impact': '原',
  'honkai-star-rail': '崩',
  'zenless-zone-zero': '绝',
  'arknights-endfield': '终',
};

interface CoverArtProps {
  gameId: GameId;
  /** 游戏中文名，用于占位文字 */
  name: string;
  /** 预留：将来接入已授权的官方素材 */
  src?: string;
  altText?: string;
  className?: string;
  /** 是否显示中央字形水印 */
  showGlyph?: boolean;
}

export const CoverArt: React.FC<CoverArtProps> = ({
  gameId,
  name,
  src,
  altText,
  className = '',
  showGlyph = true,
}) => {
  if (src) {
    return (
      <img
        src={src}
        alt={altText ?? name}
        className={`w-full h-full object-cover ${className}`}
        loading="lazy"
      />
    );
  }

  return (
    <div
      className={`relative w-full h-full bg-gradient-to-br ${GRADIENT[gameId]} ${className}`}
      role="img"
      aria-label={`${name} 封面占位`}
    >
      {/* 细密网格，呼应设计系统的工程质感 */}
      <div
        className="absolute inset-0 opacity-[0.07]"
        style={{
          backgroundImage:
            'linear-gradient(rgba(255,255,255,.5) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,.5) 1px, transparent 1px)',
          backgroundSize: '24px 24px',
        }}
      />
      {showGlyph && (
        <div className="absolute inset-0 flex items-center justify-center">
          <span className="text-[64px] font-semibold leading-none text-white/[0.07] select-none">
            {GLYPH[gameId]}
          </span>
        </div>
      )}
    </div>
  );
};
