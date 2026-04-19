// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

// Configurable mock state
let mockState: Record<string, unknown> = {};
function setMockState(overrides: Record<string, unknown>) {
  mockState = {
    teamMembers: [],
    teamStatus: null,
    ...overrides,
  };
}

vi.mock('../../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

import { TeamMemberList } from '../TeamMemberList';

describe('TeamMemberList', () => {
  it('shows empty state when no members exist', () => {
    setMockState({ teamMembers: [] });
    render(<TeamMemberList />);
    expect(screen.getByText('team.members.empty')).toBeInTheDocument();
  });

  it('renders a team member with display name', () => {
    setMockState({
      teamMembers: [
        { client_id: 'user-1', display_name: 'Alice', role: 'admin', last_seen: null },
      ],
      teamStatus: { client_id: 'other-user' },
    });
    render(<TeamMemberList />);
    expect(screen.getByText('Alice')).toBeInTheDocument();
  });

  it('shows the initial letter as avatar', () => {
    setMockState({
      teamMembers: [
        { client_id: 'user-1', display_name: 'Bob', role: 'member', last_seen: null },
      ],
      teamStatus: { client_id: 'other-user' },
    });
    render(<TeamMemberList />);
    expect(screen.getByText('B')).toBeInTheDocument();
  });

  it('shows "you" badge for the current user', () => {
    setMockState({
      teamMembers: [
        { client_id: 'user-1', display_name: 'Alice', role: 'admin', last_seen: null },
      ],
      teamStatus: { client_id: 'user-1' },
    });
    render(<TeamMemberList />);
    expect(screen.getByText('team.members.you')).toBeInTheDocument();
  });

  it('does not show "you" badge for other users', () => {
    setMockState({
      teamMembers: [
        { client_id: 'user-2', display_name: 'Bob', role: 'member', last_seen: null },
      ],
      teamStatus: { client_id: 'user-1' },
    });
    render(<TeamMemberList />);
    expect(screen.queryByText('team.members.you')).not.toBeInTheDocument();
  });

  it('displays the member role', () => {
    setMockState({
      teamMembers: [
        { client_id: 'user-1', display_name: 'Alice', role: 'admin', last_seen: null },
      ],
      teamStatus: null,
    });
    render(<TeamMemberList />);
    expect(screen.getByText('admin')).toBeInTheDocument();
  });

  it('renders multiple team members', () => {
    setMockState({
      teamMembers: [
        { client_id: 'user-1', display_name: 'Alice', role: 'admin', last_seen: null },
        { client_id: 'user-2', display_name: 'Bob', role: 'member', last_seen: null },
        { client_id: 'user-3', display_name: 'Charlie', role: 'member', last_seen: null },
      ],
      teamStatus: null,
    });
    render(<TeamMemberList />);
    expect(screen.getByText('Alice')).toBeInTheDocument();
    expect(screen.getByText('Bob')).toBeInTheDocument();
    expect(screen.getByText('Charlie')).toBeInTheDocument();
  });
});
