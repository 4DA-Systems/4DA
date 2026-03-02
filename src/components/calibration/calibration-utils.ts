export interface PullProgress {
  model: string;
  status: string;
  percent: number;
  done: boolean;
}

export function gradeColor(grade: string): string {
  if (grade.startsWith('A')) return '#22C55E';
  if (grade.startsWith('B')) return '#D4AF37';
  if (grade.startsWith('C')) return '#F59E0B';
  if (grade.startsWith('D')) return '#EF4444';
  return '#EF4444';
}

export function priorityColor(p: string): string {
  if (p === 'P0') return '#EF4444';
  if (p === 'P1') return '#F59E0B';
  return '#666666';
}
