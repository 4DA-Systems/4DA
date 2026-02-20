import { StackSelectStep } from './StackSelectStep';

interface SetupStackProps {
  selectedStacks: string[];
  onSelectionChange: (stacks: string[]) => void;
}

export function SetupStack({
  selectedStacks,
  onSelectionChange,
}: SetupStackProps) {
  return (
    <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border">
      <StackSelectStep
        selected={selectedStacks}
        onSelectionChange={onSelectionChange}
        compact
      />
    </div>
  );
}
