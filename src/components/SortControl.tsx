import { SlidersHorizontal } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { PlayOrder } from '../lib/backend/types';
import { Select } from './Select';

interface SortControlProps {
  sort: PlayOrder;
  descending: boolean;
  allowManual?: boolean;
  onChange: (sort: PlayOrder, descending: boolean) => void;
}

interface SortChoice {
  sort: PlayOrder;
  descending: boolean;
  key: string;
}

const choices: SortChoice[] = [
  { sort: 'manual', descending: false, key: 'savedOrder' },
  { sort: 'title', descending: false, key: 'titleAZ' },
  { sort: 'title', descending: true, key: 'titleZA' },
  { sort: 'artist', descending: false, key: 'artistAZ' },
  { sort: 'artist', descending: true, key: 'artistZA' },
  { sort: 'album', descending: false, key: 'albumAZ' },
  { sort: 'album', descending: true, key: 'albumZA' },
  { sort: 'filename', descending: false, key: 'filenameAZ' },
  { sort: 'filename', descending: true, key: 'filenameZA' },
  { sort: 'year', descending: false, key: 'yearOldest' },
  { sort: 'year', descending: true, key: 'yearNewest' },
  { sort: 'duration', descending: false, key: 'durationShortest' },
  { sort: 'duration', descending: true, key: 'durationLongest' },
  // The library comparator's default "added" order is newest first.
  { sort: 'added', descending: false, key: 'addedNewest' },
  { sort: 'added', descending: true, key: 'addedOldest' },
];

const choiceValue = (choice: Pick<SortChoice, 'sort' | 'descending'>) =>
  `${choice.sort}:${choice.descending ? 'desc' : 'asc'}`;

/** Select the criterion and its explicit direction in one operation. */
export function SortControl({ sort, descending, allowManual = false, onChange }: SortControlProps) {
  const { t } = useTranslation();
  const available = choices.filter(choice => choice.sort !== 'manual' || allowManual);

  return <Select
    className="library-sort"
    leadingIcon={<SlidersHorizontal size={14} />}
    value={choiceValue({ sort, descending: sort === 'manual' ? false : descending })}
    label={t('library.sort')}
    options={available.map(choice => ({ value: choiceValue(choice), label: t(`library.sortOptions.${choice.key}`) }))}
    onChange={value => {
      const choice = available.find(option => choiceValue(option) === value);
      if (choice) onChange(choice.sort, choice.descending);
    }}
  />;
}
