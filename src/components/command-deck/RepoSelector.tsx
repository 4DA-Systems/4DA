import { useEffect } from 'react';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';

export function RepoSelector() {
  const { repos, selectedRepoPath, loadRepos, setSelectedRepo } = useAppStore(
    useShallow((s) => ({
      repos: s.repos,
      selectedRepoPath: s.selectedRepoPath,
      loadRepos: s.loadRepos,
      setSelectedRepo: s.setSelectedRepo,
    })),
  );

  useEffect(() => {
    if (repos.length === 0) loadRepos();
  }, [repos.length, loadRepos]);

  if (repos.length === 0) {
    return <span className="text-xs text-gray-500">No repos found</span>;
  }

  return (
    <select
      value={selectedRepoPath || ''}
      onChange={(e) => setSelectedRepo(e.target.value)}
      aria-label="Select repository"
      className="bg-bg-tertiary border border-border rounded px-2 py-1 text-sm text-white max-w-[200px] truncate focus:outline-none focus:border-gray-500"
    >
      {repos.map((repo) => (
        <option key={repo.path} value={repo.path}>
          {repo.name} {repo.has_changes ? '*' : ''}
        </option>
      ))}
    </select>
  );
}
