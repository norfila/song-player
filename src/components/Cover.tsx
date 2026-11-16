import { Disc3, Music2 } from 'lucide-react';
import { assetUrl } from '../lib/backend';
import type { Track } from '../lib/backend/types';
import { useState } from 'react';

export function Cover({ track, size = 'small', album = false }: { track?: Track | null; size?: 'small' | 'medium' | 'large'; album?: boolean }) {
  const [failed, setFailed] = useState<string | null>(null);
  const path = track?.coverPath;
  return <span className={`cover cover-${size} ${album ? 'cover-album' : ''}`} aria-hidden="true">
    {path && failed !== path ? <img src={assetUrl(path)} alt="" onError={() => setFailed(path)} /> : album ? <Disc3 /> : <Music2 />}
  </span>;
}
