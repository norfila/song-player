import { Coffee, Disc3, Folder, Headphones, Heart, Moon, Music2, Star, Sun, Zap } from 'lucide-react';
import type { PlaylistIcon as IconValue } from '../lib/backend/types';
import { assetUrl } from '../lib/backend';
export const PLAYLIST_ICONS = { music: Music2, headphones: Headphones, heart: Heart, star: Star, disc: Disc3, folder: Folder, moon: Moon, sun: Sun, zap: Zap, coffee: Coffee };
export function PlaylistIcon({ icon, size = 20 }: { icon?: IconValue; size?: number }) {
  if (icon?.kind === 'image') return <img src={assetUrl(icon.value)} alt="" className="playlist-custom-icon" style={{ width: size, height: size }} />;
  const Icon = PLAYLIST_ICONS[icon?.value as keyof typeof PLAYLIST_ICONS] ?? Music2;
  return <Icon size={size} aria-hidden="true" />;
}
